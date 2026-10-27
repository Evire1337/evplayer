import { makeImpulse } from "./impulse";
import { semitonesToRatio } from "./pitch";

// The sound path for the player:
//   <audio> -> pitch shifter -> dry ───────────────┐
//                            └-> reverb -> wet ────┴-> output -> speakers
// It is built the first time an effect is needed, so plain playback never pays for it.

class AudioGraph {
  private ctx: AudioContext | null = null;
  private shifter: AudioWorkletNode | null = null;
  private dry: GainNode | null = null;
  private wet: GainNode | null = null;
  private building: Promise<void> | null = null;

  constructor(private readonly audio: HTMLAudioElement) {}

  get active(): boolean {
    return this.ctx !== null;
  }

  private build(): Promise<void> {
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

      source.connect(shifter);
      shifter.connect(dry).connect(ctx.destination);
      shifter.connect(highpass).connect(convolver).connect(wet).connect(ctx.destination);

      this.ctx = ctx;
      this.shifter = shifter;
      this.dry = dry;
      this.wet = wet;
    })();
    return this.building;
  }

  async apply(effects: { pitchSemitones: number; reverb: boolean; reverbAmount: number }): Promise<void> {
    const needed = Math.abs(effects.pitchSemitones) > 0.01 || effects.reverb;
    if (!needed && !this.ctx) return;
    await this.build();
    const ctx = this.ctx!;
    if (ctx.state === "suspended") await ctx.resume().catch(() => {});
    const now = ctx.currentTime;
    this.shifter!.parameters.get("ratio")!.setTargetAtTime(semitonesToRatio(effects.pitchSemitones), now, 0.02);
    const amount = effects.reverb ? Math.min(1, Math.max(0, effects.reverbAmount)) : 0;
    // equal-power-ish blend: the room grows without the song getting louder
    this.wet!.gain.setTargetAtTime(amount * 0.9, now, 0.05);
    this.dry!.gain.setTargetAtTime(1 - amount * 0.35, now, 0.05);
  }

  resume(): void {
    if (this.ctx?.state === "suspended") void this.ctx.resume().catch(() => {});
  }
}

export type { AudioGraph };
export function createAudioGraph(audio: HTMLAudioElement): AudioGraph {
  return new AudioGraph(audio);
}
