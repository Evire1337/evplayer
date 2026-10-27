import { makeImpulse } from "./impulse";
import { semitonesToRatio } from "./pitch";
import { createEqBus } from "@/equalizer/eqAudio";

// The sound path for the player:
//   <audio> -> pitch shifter -> dry ─────────────┐
//                            └-> reverb -> wet ──┴-> equalizer (levelling, bands, limiter) -> speakers
// It is built on the first play (the browser needs a click before it may make sound).

export interface EffectSettings {
  pitchSemitones: number;
  reverb: boolean;
  reverbAmount: number;
}

class AudioGraph {
  private ctx: AudioContext | null = null;
  private shifter: AudioWorkletNode | null = null;
  private dry: GainNode | null = null;
  private wet: GainNode | null = null;
  private building: Promise<void> | null = null;
  private pending: EffectSettings = { pitchSemitones: 0, reverb: false, reverbAmount: 0.5 };

  constructor(private readonly audio: HTMLAudioElement) {}

  get active(): boolean {
    return this.ctx !== null;
  }

  ensure(): Promise<void> {
    this.building ??= (async () => {
      const ctx = new AudioContext({ latencyHint: "playback" });
      await ctx.audioWorklet.addModule("./worklets/pitch-shifter.js");
      const source = ctx.createMediaElementSource(this.audio);
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

      source.connect(shifter);
      shifter.connect(dry).connect(eq.input);
      shifter.connect(highpass).connect(convolver).connect(wet).connect(eq.input);
      eq.output.connect(ctx.destination);

      this.ctx = ctx;
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
    this.shifter!.parameters.get("ratio")!.setTargetAtTime(semitonesToRatio(fx.pitchSemitones), now, 0.02);
    const amount = fx.reverb ? Math.min(1, Math.max(0, fx.reverbAmount)) : 0;
    this.wet!.gain.setTargetAtTime(amount * 0.9, now, 0.05);
    this.dry!.gain.setTargetAtTime(1 - amount * 0.35, now, 0.05);
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
export function createAudioGraph(audio: HTMLAudioElement): AudioGraph {
  return new AudioGraph(audio);
}
