import { makeImpulse } from "./impulse";
import { semitonesToRatio } from "./pitch";
import { createEqBus } from "@/equalizer/eqAudio";

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
  private pendingGain = new Map<HTMLAudioElement, number>();

  constructor(private readonly decks: HTMLAudioElement[]) {}

  get active(): boolean {
    return this.ctx !== null;
  }

  ensure(): Promise<void> {
    this.building ??= (async () => {
      const ctx = new AudioContext({ latencyHint: "playback" });
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
      for (const audio of this.decks) {
        const gain = ctx.createGain();
        gain.gain.value = this.pendingGain.get(audio) ?? 1;
        ctx.createMediaElementSource(audio).connect(gain).connect(mix);
        this.gains.set(audio, gain);
        audio.addEventListener("play", this.onPlay);
        audio.addEventListener("pause", this.onPause);
        audio.addEventListener("ended", this.onPause);
      }
      mix.connect(post);
      post.connect(dry).connect(eq.input);
      highpass.connect(convolver).connect(wet).connect(eq.input);
      eq.output.connect(ctx.destination);

      this.ctx = ctx;
      this.mix = mix;
      this.post = post;
      this.reverbIn = highpass;
      this.shifter = shifter;
      this.dry = dry;
      this.wet = wet;
      this.write(this.pending);
    })();
    return this.building;
  }

  private write(fx: EffectSettings): void {
    if (!this.ctx) return;
    const now = this.ctx.currentTime;
    const ratio = semitonesToRatio(fx.pitchSemitones);
    this.shifter!.parameters.get("ratio")!.setTargetAtTime(ratio, now, 0.02);
    this.wireShifter(Math.abs(ratio - 1) > 1e-4);
    const amount = fx.reverb ? Math.min(1, Math.max(0, fx.reverbAmount)) : 0;
    this.wireReverb(amount > 0);
    this.wet!.gain.setTargetAtTime(amount * 0.9, now, 0.05);
    this.dry!.gain.setTargetAtTime(1 - amount * 0.35, now, 0.05);
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
    if (this.ctx?.state === "suspended") void this.ctx.resume();
  };

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
