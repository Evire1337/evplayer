// Stem player (AudioWorklet). The four tracks of a split song (vocals, drums, bass, other) play
// from one node on the sound card's own clock, so they can never drift apart or stall one by
// one the way four separate <audio> elements did. The page feeds it the sound in chunks of
// 16-bit stereo (44.1 kHz, as the files are) and tells it where to play from, how fast, and
// when to stop; it answers with where it is.
//
// Output: 8 channels = 4 stereo pairs, one pair per track in the order vocals, drums, bass,
// other. The levels, the speed/pitch chain and the equalizer are the page's graph's job.
//
// - Positions are in source frames (44.1 kHz) and may be fractional: the playing speed and the
//   difference between 44.1 kHz and the context's rate are both one step per output frame, read
//   with a cubic (Catmull-Rom) interpolation. At speed 1 on a 44.1 kHz context it reads the
//   samples exactly.
// - Only a few chunks are held at once (the page says which: `keep`), so memory stays small.
// - If a chunk the next frames need has not arrived, the node outputs silence and does not
//   advance (`starved`) until it does: nothing is skipped and nothing is played twice.
// - Starting, stopping and seeking go through a short fade, so no click is heard.

const SOURCE_RATE = 44100;
/** frames per chunk: a power of two, so the chunk of a frame is a shift */
const CHUNK_BITS = 17;
const CHUNK = 1 << CHUNK_BITS;
const CHUNK_MASK = CHUNK - 1;
/** chunks held at once (the page keeps fewer than this) */
const SLOTS = 16;
const PARTS = 4;
/** frames a fade in or out takes: about 12 ms */
const FADE_FRAMES = 512;
/** how often the position is reported while playing, in output frames: about 40 ms */
const REPORT_FRAMES = 2048;

class StemPlayer extends AudioWorkletProcessor {
  constructor() {
    super();
    /** slot = chunk index & (SLOTS - 1): { index, parts: Int16Array[4] (interleaved stereo), frames } */
    this.slots = new Array(SLOTS).fill(null);
    /** where the next output frame reads, in source frames */
    this.pos = 0;
    /** the length of the song in frames; Infinity until the page says */
    this.length = Infinity;
    /** the playing speed (1 = as recorded) */
    this.rate = 1;
    this.playing = false;
    /** 0 to 1: the fade that takes the clicks out of starting, stopping and seeking */
    this.level = 0;
    this.starved = false;
    this.ended = false;
    /** a seek that waits for the sound to fade out first (null: none) */
    this.pendingSeek = null;
    /** what must not change while the sound fades out of the place it is at: chunks to let go, chunks that would take a slot */
    this.pendingKeep = null;
    this.queued = [];
    this.sinceReport = 0;
    this.port.onmessage = (event) => this.onMessage(event.data);
  }

  onMessage(msg) {
    switch (msg.type) {
      case "chunk": {
        const parts = msg.parts.map((buffer) => new Int16Array(buffer));
        const frames = Math.min(CHUNK, ...parts.map((p) => p.length >> 1));
        const slot = { index: msg.index, parts, frames };
        const taken = this.slots[msg.index & (SLOTS - 1)];
        // a slot that the sound is still reading from waits
        if (taken && taken.index !== msg.index && this.holds(taken)) this.queued.push(slot);
        else this.slots[msg.index & (SLOTS - 1)] = slot;
        break;
      }
      case "keep":
        // chunks outside [from, to] are let go, once the sound no longer reads them
        this.pendingKeep = { from: msg.from, to: msg.to };
        this.settle();
        break;
      case "reset":
        this.slots.fill(null);
        this.pendingKeep = null;
        this.queued = [];
        this.pos = 0;
        this.length = Infinity;
        this.playing = false;
        this.level = 0;
        this.ended = false;
        this.starved = false;
        this.pendingSeek = null;
        break;
      case "length":
        this.length = msg.frames;
        break;
      case "seek":
        // what plays now fades out first, then the song goes on from the new place
        if (this.level > 0) this.pendingSeek = Math.max(0, msg.frame);
        else {
          this.pos = Math.max(0, msg.frame);
          this.level = 0;
          this.pendingSeek = null;
        }
        this.ended = false;
        this.report();
        break;
      case "play":
        this.playing = !!msg.on;
        if (this.playing) this.ended = false;
        this.report();
        break;
      case "rate":
        this.rate = msg.value > 0 ? msg.value : 1;
        break;
    }
  }

  /** the chunk is one the sound reads from now (while it plays or fades out) */
  holds(slot) {
    if (this.level <= 0) return false;
    const at = Math.floor(this.pos);
    return slot.index >= Math.max(0, at - 1) >> CHUNK_BITS && slot.index <= (at + 2) >> CHUNK_BITS;
  }

  /** lets go of the chunks outside the window and takes in the ones that waited, except where the sound is reading */
  settle() {
    const keep = this.pendingKeep;
    if (keep) {
      let held = false;
      for (let i = 0; i < SLOTS; i++) {
        const slot = this.slots[i];
        if (!slot || (slot.index >= keep.from && slot.index <= keep.to)) continue;
        if (this.holds(slot)) held = true;
        else this.slots[i] = null;
      }
      if (!held) this.pendingKeep = null;
    }
    const queued = this.queued;
    this.queued = [];
    for (const slot of queued) {
      const taken = this.slots[slot.index & (SLOTS - 1)];
      if (taken && taken.index !== slot.index && this.holds(taken)) this.queued.push(slot);
      else this.slots[slot.index & (SLOTS - 1)] = slot;
    }
  }

  report() {
    this.sinceReport = 0;
    this.port.postMessage({ type: "pos", frame: this.pos, at: currentTime, playing: this.playing, starved: this.starved });
  }

  /** the chunk holding source frame `i`, or null if it is not here (offset = where in it) */
  slotOf(i) {
    const index = i >> CHUNK_BITS;
    const slot = this.slots[index & (SLOTS - 1)];
    return slot && slot.index === index ? slot : null;
  }

  process(_inputs, outputs) {
    const out = outputs[0];
    if (!out || out.length === 0) return true;
    const frames = out[0].length;
    if (!this.playing && this.level <= 0) {
      for (let c = 0; c < out.length; c++) out[c].fill(0);
      return true;
    }

    const step = (SOURCE_RATE / sampleRate) * this.rate;
    const fadeStep = 1 / FADE_FRAMES;
    const wasStarved = this.starved;
    const channels = Math.min(out.length, PARTS * 2);

    for (let n = 0; n < frames; n++) {
      const i1 = Math.floor(this.pos);
      // while a stop fades out the song goes on being read, so the fade is the song's own sound
      let readable = (this.playing || this.level > 0) && !this.ended;
      let s0 = null;
      let s1 = null;
      let s2 = null;
      let s3 = null;
      if (readable) {
        if (i1 >= this.length) {
          readable = false;
          this.ended = true;
          this.playing = false;
          this.port.postMessage({ type: "end" });
        } else {
          // the four frames the interpolation reads; before the start and after the end is silence
          s1 = this.slotOf(i1);
          s0 = i1 >= 1 ? this.slotOf(i1 - 1) : null;
          s2 = i1 + 1 < this.length ? this.slotOf(i1 + 1) : null;
          s3 = i1 + 2 < this.length ? this.slotOf(i1 + 2) : null;
          const missing =
            s1 === null || (i1 >= 1 && s0 === null && i1 - 1 < this.length) || (i1 + 1 < this.length && s2 === null) || (i1 + 2 < this.length && s3 === null);
          if (missing) {
            readable = false;
            this.starved = true;
          } else {
            this.starved = false;
          }
        }
      }

      const target = readable && this.playing && this.pendingSeek === null ? 1 : 0;
      if (this.level < target) this.level = Math.min(target, this.level + fadeStep);
      else if (this.level > target) this.level = Math.max(target, this.level - fadeStep);
      if (this.pendingSeek !== null && (this.level <= 0 || !readable)) {
        // faded out (or there was nothing to fade): the new place
        this.pos = this.pendingSeek;
        this.pendingSeek = null;
        this.level = 0;
        this.settle();
        for (let c = 0; c < channels; c++) out[c][n] = 0;
        continue;
      }

      if (!readable) {
        // nothing to read (a chunk is missing, or the song is over): silence, and the fade starts again from it
        for (let c = 0; c < channels; c++) out[c][n] = 0;
        continue;
      }

      const t = this.pos - i1;
      const o0 = (i1 - 1) & CHUNK_MASK;
      const o1 = i1 & CHUNK_MASK;
      const o2 = (i1 + 1) & CHUNK_MASK;
      const o3 = (i1 + 2) & CHUNK_MASK;
      const level = this.level / 32768;
      for (let p = 0; p < PARTS; p++) {
        for (let ch = 0; ch < 2; ch++) {
          const a = s0 !== null && o0 < s0.frames && i1 >= 1 ? s0.parts[p][o0 * 2 + ch] : 0;
          const b = s1 !== null && o1 < s1.frames ? s1.parts[p][o1 * 2 + ch] : 0;
          const c = s2 !== null && o2 < s2.frames ? s2.parts[p][o2 * 2 + ch] : 0;
          const d = s3 !== null && o3 < s3.frames ? s3.parts[p][o3 * 2 + ch] : 0;
          const value = t === 0 ? b : 0.5 * (2 * b + (-a + c) * t + (2 * a - 5 * b + 4 * c - d) * t * t + (-a + 3 * b - 3 * c + d) * t * t * t);
          out[p * 2 + ch][n] = value * level;
        }
      }
      this.pos += step;
    }

    if (this.pendingKeep || this.queued.length) this.settle();
    this.sinceReport += frames;
    if (this.sinceReport >= REPORT_FRAMES || this.starved !== wasStarved) this.report();
    return true;
  }
}

registerProcessor("evplayer-stem-player", StemPlayer);
