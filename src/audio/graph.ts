import { createMeters, type BandMeters } from "./meters";
import { createSoundMixer, FLAT, isFlat, type MixerLevels, type SoundMixerStage } from "./soundMixer";
import { makeImpulse } from "./impulse";
import { semitonesToRatio } from "./pitch";
import { pitchCompensation } from "@/stems/pcm";
import { createEqBus } from "@/equalizer/eqAudio";
import { isPhone } from "@/platform/device";

// The sound path for the player (two <audio> "decks", so the next song can be ready and
// fade in over the end of the current one):
//   deck A -> gain ┐
//   deck B -> gain ┴-> pitch shifter -> dry ─────────────┐
//                            └-> reverb -> wet ──┴-> equalizer (levelling, bands, limiter) -> speakers
// It is built on the first play (the browser needs a click before it may make sound).
//
// Saving work: the pitch shifter and the reverb are only wired in while they are used (a
// connected node keeps computing even when it changes nothing; the reverb is the costliest
// part of the whole graph), and the whole graph is suspended after a while on pause.

/** how long a paused player keeps its sound graph running before suspending it */
const SUSPEND_AFTER_MS = 8000;

export interface EffectSettings {
  pitchSemitones: number;
  reverb: boolean;
  reverbAmount: number;
}

/** what the page holds of the split tracks' player (see stems/player.ts) */
export interface StemSink {
  port: MessagePort;
  /** the levels of the four tracks (vocals, drums, bass, other) */
  setGains: (values: number[]) => void;
  /** the sound graph's clock, in seconds */
  now: () => number;
  /** takes the player out of the graph and lets its memory go */
  dispose: () => void;
}

class AudioGraph {
  private ctx: AudioContext | null = null;
  private shifter: AudioWorkletNode | null = null;
  /** the decks meet here; goes through the shifter only while the pitch is changed */
  private mix: GainNode | null = null;
  /** after the shifter: feeds the dry path and, while reverb is on, the reverb */
  private post: GainNode | null = null;
  private reverbIn: BiquadFilterNode | null = null;
  private shifterOn = false;
  private reverbOn = false;
  private reverbOffTimer: ReturnType<typeof setTimeout> | null = null;
  private suspendTimer: ReturnType<typeof setTimeout> | null = null;
  private dry: GainNode | null = null;
  private wet: GainNode | null = null;
  private building: Promise<void> | null = null;
  private pending: EffectSettings = { pitchSemitones: 0, reverb: false, reverbAmount: 0.5 };
  private gains = new Map<HTMLAudioElement, GainNode>();
  /** a deck's switch: shut while the four split tracks play in its place */
  private gates = new Map<HTMLAudioElement, GainNode>();
  /** the player of the four split tracks (one node, four pairs of channels), made once and kept until let go */
  private stem: StemSink | null = null;
  private stemBuilding: Promise<StemSink | null> | null = null;
  /** the levels of the four tracks, kept for a player that is not made yet */
  private stemLevels: number[] = [1, 1, 1, 1];
  /** semitones the shifter adds so a slower or faster song keeps its pitch (the tracks play like a record) */
  private stemPitch = 0;
  private pendingGain = new Map<HTMLAudioElement, number>();
  /** the decks meet here, before the vocal cut and everything else */
  private deckBus: GainNode | null = null;
  private mixer: SoundMixerStage | null = null;
  private mixerOn = false;
  private pendingMixer: MixerLevels = FLAT;
  private meters: BandMeters | null = null;
  /** the volume knob, last in the chain: some systems (WebKitGTK on Linux) ignore the element's own volume once its sound goes through the graph, so 0 still played */
  private master: GainNode | null = null;
  private pendingMaster = 1;

  constructor(private readonly decks: HTMLAudioElement[]) {}

  get active(): boolean {
    return this.ctx !== null;
  }

  ensure(): Promise<void> {
    this.building ??= (async () => {
      // a phone gets a long buffer: in the background Android slows the page down, and a
      // short one ran dry and crackled (the lyrics take the delay into account)
      const ctx = new AudioContext({ latencyHint: isPhone ? 0.3 : "playback" });
      await ctx.audioWorklet.addModule("./worklets/pitch-shifter.js");
      const shifter = new AudioWorkletNode(ctx, "evplayer-pitch-shifter", { outputChannelCount: [2] });
      const dry = ctx.createGain();
      const wet = ctx.createGain();
      wet.gain.value = 0;
      const convolver = ctx.createConvolver();
      convolver.buffer = makeImpulse(ctx);
      // keep the low end out of the reverb so bass and kicks stay tight
      const highpass = ctx.createBiquadFilter();
      highpass.type = "highpass";
      highpass.frequency.value = 160;
      const eq = createEqBus(ctx);

      const mix = ctx.createGain();
      const post = ctx.createGain();
      const deckBus = ctx.createGain();
      deckBus.connect(mix);
      for (const audio of this.decks) {
        const gain = ctx.createGain();
        gain.gain.value = this.pendingGain.get(audio) ?? 1;
        const gate = ctx.createGain();
        ctx.createMediaElementSource(audio).connect(gain).connect(gate).connect(deckBus);
        this.gains.set(audio, gain);
        this.gates.set(audio, gate);
        audio.addEventListener("play", this.onPlay);
        audio.addEventListener("pause", this.onPause);
        audio.addEventListener("ended", this.onPause);
      }
      mix.connect(post);
      post.connect(dry).connect(eq.input);
      highpass.connect(convolver).connect(wet).connect(eq.input);
      const master = ctx.createGain();
      master.gain.value = this.pendingMaster;
      eq.output.connect(master).connect(ctx.destination);
      // from now on the knob is the graph's: the elements play at full level into it
      for (const audio of this.decks) audio.volume = 1;

      this.ctx = ctx;
      this.deckBus = deckBus;
      this.master = master;
      this.mix = mix;
      this.post = post;
      this.reverbIn = highpass;
      this.shifter = shifter;
      this.dry = dry;
      this.wet = wet;
      this.write(this.pending);
      this.setMixer(this.pendingMixer);
    })();
    return this.building;
  }

  private write(fx: EffectSettings): void {
    if (!this.ctx) return;
    const now = this.ctx.currentTime;
    const ratio = semitonesToRatio(fx.pitchSemitones + this.stemPitch);
    this.shifter!.parameters.get("ratio")!.setTargetAtTime(ratio, now, 0.02);
    this.wireShifter(Math.abs(ratio - 1) > 1e-4);
    const amount = fx.reverb ? Math.min(1, Math.max(0, fx.reverbAmount)) : 0;
    this.wireReverb(amount > 0);
    this.wet!.gain.setTargetAtTime(amount * 0.9, now, 0.05);
    this.dry!.gain.setTargetAtTime(1 - amount * 0.35, now, 0.05);
  }

  /** taps that show what each part of the song carries (for the "Дорожки" page); null until the graph runs */
  openMeters(): BandMeters | null {
    if (!this.ctx || !this.deckBus) return null;
    this.meters ??= createMeters(this.ctx, this.deckBus);
    return this.meters;
  }

  closeMeters(): void {
    this.meters?.dispose();
    this.meters = null;
  }

  /** the voice, bass and width levels of every song (audio/soundMixer.ts); costs nothing while all are 100% */
  setMixer(levels: MixerLevels): void {
    this.pendingMixer = levels;
    if (!this.ctx || !this.deckBus || !this.mix) return;
    const on = !isFlat(levels);
    if (on) {
      this.mixer ??= createSoundMixer(this.ctx);
      this.mixer.setLevels(levels);
    }
    if (on === this.mixerOn) return;
    this.mixerOn = on;
    if (on && this.mixer) {
      this.deckBus.disconnect(this.mix);
      this.deckBus.connect(this.mixer.input);
      this.mixer.output.connect(this.mix);
    } else if (this.mixer) {
      this.deckBus.disconnect(this.mixer.input);
      this.mixer.output.disconnect(this.mix);
      this.deckBus.connect(this.mix);
    }
  }

  private wireShifter(on: boolean): void {
    if (on === this.shifterOn || !this.mix || !this.post || !this.shifter) return;
    this.shifterOn = on;
    if (on) {
      this.mix.connect(this.shifter).connect(this.post);
      this.mix.disconnect(this.post);
    } else {
      this.mix.connect(this.post);
      this.mix.disconnect(this.shifter);
      this.shifter.disconnect();
    }
  }

  private wireReverb(on: boolean): void {
    if (!this.post || !this.reverbIn) return;
    if (on) {
      if (this.reverbOffTimer) clearTimeout(this.reverbOffTimer);
      this.reverbOffTimer = null;
      if (!this.reverbOn) this.post.connect(this.reverbIn);
      this.reverbOn = true;
    } else if (this.reverbOn && !this.reverbOffTimer) {
      // let the tail fade out before cutting the reverb off
      this.reverbOffTimer = setTimeout(() => {
        this.reverbOffTimer = null;
        this.reverbOn = false;
        this.post?.disconnect(this.reverbIn!);
      }, 2500);
    }
  }

  private onPlay = (): void => {
    if (this.suspendTimer) clearTimeout(this.suspendTimer);
    this.suspendTimer = null;
    this.wake(3);
  };

  /**
   * Resume the suspended sound graph. Woken from the background (a phone's notification
   * button) the first try can be refused, and the song then "played" without a sound: try
   * again a few times.
   */
  private wake(tries: number): void {
    const ctx = this.ctx;
    if (!ctx || ctx.state === "running" || ctx.state === "closed") return;
    void ctx.resume().catch(() => undefined).finally(() => {
      if (ctx.state !== "running" && tries > 1 && this.decks.some((d) => !d.paused)) setTimeout(() => this.wake(tries - 1), 400);
    });
  }

  private onPause = (): void => {
    if (this.decks.some((d) => !d.paused)) return;
    if (this.suspendTimer) clearTimeout(this.suspendTimer);
    this.suspendTimer = setTimeout(() => {
      this.suspendTimer = null;
      if (this.decks.every((d) => d.paused) && this.ctx?.state === "running") void this.ctx.suspend();
    }, SUSPEND_AFTER_MS);
  };

  /**
   * how far the sound you hear is behind the song's position: the sound card's buffer and
   * the system's output path (lyrics wait for it, or they run ahead of the singer)
   */
  get latencyMs(): number {
    if (!this.ctx) return 0;
    const seconds = (this.ctx.baseLatency || 0) + (this.ctx.outputLatency || 0);
    return Number.isFinite(seconds) ? Math.min(500, Math.max(0, seconds * 1000)) : 0;
  }

  /** can decks be faded against each other right now */
  get canFade(): boolean {
    return this.ctx !== null && this.ctx.state === "running";
  }

  /** shuts or opens a deck's sound without touching its level (the split tracks play instead) */
  setDeckMuted(audio: HTMLAudioElement, muted: boolean): void {
    const gate = this.gates.get(audio);
    if (gate && this.ctx) gate.gain.setTargetAtTime(muted ? 0 : 1, this.ctx.currentTime, 0.01);
  }

  /**
   * The player of the split tracks, made on first ask. Its four tracks join the decks at the
   * deck bus, each with its own level. null while the graph is not running or if the browser
   * would not take the worklet.
   */
  openStemSink(): Promise<StemSink | null> {
    if (this.stem) return Promise.resolve(this.stem);
    this.stemBuilding ??= (async () => {
      const ctx = this.ctx;
      const bus = this.deckBus;
      if (!ctx || !bus) return null;
      try {
        await ctx.audioWorklet.addModule("./worklets/stem-player.js");
      } catch {
        return null;
      }
      const node = new AudioWorkletNode(ctx, "evplayer-stem-player", { numberOfInputs: 0, numberOfOutputs: 1, outputChannelCount: [8] });
      const splitter = ctx.createChannelSplitter(8);
      node.connect(splitter);
      const gains: GainNode[] = [];
      const mergers: ChannelMergerNode[] = [];
      for (let i = 0; i < 4; i++) {
        const merger = ctx.createChannelMerger(2);
        splitter.connect(merger, i * 2, 0);
        splitter.connect(merger, i * 2 + 1, 1);
        const gain = ctx.createGain();
        gain.gain.value = this.stemLevels[i] ?? 0;
        merger.connect(gain).connect(bus);
        gains.push(gain);
        mergers.push(merger);
      }
      const sink: StemSink = {
        port: node.port,
        setGains: (values) => {
          this.stemLevels = values.slice(0, 4);
          gains.forEach((g, i) => g.gain.setTargetAtTime(values[i] ?? 0, ctx.currentTime, 0.02));
        },
        now: () => ctx.currentTime,
        dispose: () => {
          node.port.onmessage = null;
          node.port.postMessage({ type: "reset" });
          node.disconnect();
          splitter.disconnect();
          for (const m of mergers) m.disconnect();
          for (const g of gains) g.disconnect();
          if (this.stem === sink) this.stem = null;
          this.stemBuilding = null;
          this.setStemSpeed(1, false);
        },
      };
      this.stem = sink;
      return sink;
    })();
    return this.stemBuilding;
  }

  /**
   * The tracks play at the deck's speed like a record, so the pitch moves with it; where the
   * deck keeps the pitch (the speed is not linked to it), the shifter takes that back.
   */
  setStemSpeed(rate: number, preservesPitch: boolean): void {
    const semitones = pitchCompensation(rate, preservesPitch);
    if (Math.abs(semitones - this.stemPitch) < 1e-9) return;
    this.stemPitch = semitones;
    this.write(this.pending);
  }

  /** the volume (0 to 1; 0 when muted) */
  setMaster(value: number): void {
    this.pendingMaster = value;
    if (!this.master || !this.ctx) return;
    this.master.gain.cancelScheduledValues(0);
    this.master.gain.setValueAtTime(value, this.ctx.currentTime);
  }

  /** a deck's level, now */
  setDeck(audio: HTMLAudioElement, value: number): void {
    this.pendingGain.set(audio, value);
    const gain = this.gains.get(audio);
    if (!gain || !this.ctx) return;
    // from 0: a fade still running is dropped too (setting a value inside it would throw)
    gain.gain.cancelScheduledValues(0);
    gain.gain.setValueAtTime(value, this.ctx.currentTime);
  }

  /** a deck's level, gliding there over `seconds` (an equal-power curve sounds even) */
  fadeDeck(audio: HTMLAudioElement, to: number, seconds: number): void {
    this.pendingGain.set(audio, to);
    const gain = this.gains.get(audio);
    if (!gain || !this.ctx) return;
    const now = this.ctx.currentTime;
    const from = gain.gain.value;
    const steps = 32;
    const curve = new Float32Array(steps);
    for (let i = 0; i < steps; i++) {
      const t = i / (steps - 1);
      curve[i] = to > from ? from + (to - from) * Math.sin((t * Math.PI) / 2) : to + (from - to) * Math.cos((t * Math.PI) / 2);
    }
    gain.gain.cancelScheduledValues(0);
    gain.gain.setValueCurveAtTime(curve, now, Math.max(0.05, seconds));
  }

  apply(fx: EffectSettings): void {
    this.pending = fx;
    this.write(fx);
  }

  resume(): void {
    void this.ensure()
      .then(() => {
        if (this.ctx?.state === "suspended") return this.ctx.resume();
      })
      .catch((err) => console.warn("[audio]", err));
  }
}

export type { AudioGraph };
export function createAudioGraph(decks: HTMLAudioElement[]): AudioGraph {
  return new AudioGraph(decks);
}
