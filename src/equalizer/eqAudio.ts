import { MAX_BANDS, peakBoostDb, type EqBand } from "./eqModel";
import { useEqualizerStore } from "./eqStore";
import { LoudnessLeveler, rememberedLoudness, rememberLoudness } from "./loudness";

// The equalizer sits at the very end of the playback graph:
//   effects output -> preamp -> band filters -> limiter -> analyser -> speakers
// When it is switched off every filter is set to 0 dB, which makes them transparent, so
// toggling never has to rewire the graph or interrupt playback.

export interface EqBus {
  input: GainNode;
  output: AudioNode;
  analyser: AnalyserNode;
  context: AudioContext;
  filters: BiquadFilterNode[];
  preamp: GainNode;
  limiter: DynamicsCompressorNode;
  level: GainNode;
  meter: AnalyserNode;
}

let bus: EqBus | null = null;

export function getEqBus(): EqBus | null {
  return bus;
}

export function createEqBus(ctx: AudioContext): EqBus {
  // volume levelling sits first, measured on what comes in
  const level = ctx.createGain();
  const meter = ctx.createAnalyser();
  meter.fftSize = 2048;
  const preamp = ctx.createGain();
  level.connect(preamp);
  // always the most bands any mode has, so switching modes never rewires the graph; the
  // ones a mode does not use stay at 0 dB, which lets the sound through untouched
  const filters = Array.from({ length: MAX_BANDS }, () => ctx.createBiquadFilter());
  const limiter = ctx.createDynamicsCompressor();
  limiter.knee.value = 0;
  limiter.ratio.value = 20;
  limiter.attack.value = 0.003;
  limiter.release.value = 0.12;
  limiter.threshold.value = 0;
  const analyser = ctx.createAnalyser();
  analyser.fftSize = 4096;
  analyser.smoothingTimeConstant = 0.82;

  let node: AudioNode = preamp;
  for (const filter of filters) {
    node.connect(filter);
    node = filter;
  }
  node.connect(limiter);
  limiter.connect(analyser);

  const input = ctx.createGain();
  input.connect(level);
  input.connect(meter);

  bus = { input, output: analyser, analyser, context: ctx, filters, preamp, limiter, level, meter };
  applyEqualizer();
  startLeveler();
  return bus;
}

/**
 * How fast changes are heard: dragging a point follows the hand at once; switching to
 * another preset (or turning the equalizer on or off) glides over about a second, so
 * presets blend into each other instead of jumping.
 */
const QUICK = 0.015;
const GLIDE = 0.3;

function applyBand(filter: BiquadFilterNode, band: EqBand, enabled: boolean, now: number, tau: number): void {
  // the filter kind cannot glide; while it switches, the levels still do
  if (filter.type !== band.type) filter.type = band.type;
  filter.frequency.setTargetAtTime(band.frequency, now, tau);
  filter.Q.setTargetAtTime(band.q, now, tau);
  filter.gain.setTargetAtTime(enabled && band.enabled ? band.gain : 0, now, tau);
}

export function applyEqualizer(tau = QUICK): void {
  if (!bus) return;
  const { enabled, bands, preamp } = useEqualizerStore.getState();
  const now = bus.context.currentTime;
  bus.filters.forEach((filter, i) => {
    const band = bands[i];
    if (band) applyBand(filter, band, enabled, now, tau);
    else {
      // a new filter starts as a low-pass: make it a flat peak so it changes nothing
      filter.type = "peaking";
      filter.gain.setTargetAtTime(0, now, tau);
    }
  });
  const { normalize, protect } = useEqualizerStore.getState();
  // with clip protection: pull the level down by most of the boost so loud masters do not
  // clip, then let the limiter catch whatever is left. Without it the boost comes through
  // at full strength and only the preamp sets the level
  const headroom = enabled ? (protect ? -peakBoostDb(bands) * 0.75 : 0) + preamp : 0;
  bus.preamp.gain.setTargetAtTime(Math.pow(10, headroom / 20), now, Math.max(tau, 0.02));
  // a ratio of 1 makes the compressor do nothing at all
  bus.limiter.ratio.setTargetAtTime(protect ? 20 : 1, now, 0.02);
  bus.limiter.threshold.setTargetAtTime(protect && (enabled || normalize) ? -1 : 0, now, 0.02);
  if (!normalize) bus.level.gain.setTargetAtTime(1, now, 0.3);
}

useEqualizerStore.subscribe((s, prev) => {
  const switched = s.preset !== prev.preset && s.preset !== "custom";
  applyEqualizer(switched || s.enabled !== prev.enabled || s.mode !== prev.mode ? GLIDE : QUICK);
});

// ---- volume levelling ---------------------------------------------------------------

const TICK_MS = 150;
let leveler = new LoudnessLeveler();
let levelTrackId: string | null = null;
let levelTimer: ReturnType<typeof setInterval> | null = null;
let samples: Float32Array<ArrayBuffer> | null = null;

/** the player calls this when a new song starts loading */
export function setLevelingTrack(trackId: string | null): void {
  if (levelTrackId && leveler.isSettled()) {
    const db = leveler.loudnessDb();
    if (db !== null) rememberLoudness(levelTrackId, db);
  }
  levelTrackId = trackId;
  leveler = new LoudnessLeveler(trackId ? rememberedLoudness(trackId) : undefined);
  if (bus && useEqualizerStore.getState().normalize) {
    bus.level.gain.setTargetAtTime(Math.pow(10, leveler.gainDb() / 20), bus.context.currentTime, 0.05);
  }
}

function tick(): void {
  if (!bus || !useEqualizerStore.getState().normalize) return;
  const meter = bus.meter;
  if (!samples || samples.length !== meter.fftSize) samples = new Float32Array(meter.fftSize);
  meter.getFloatTimeDomainData(samples);
  let sum = 0;
  for (let i = 0; i < samples.length; i++) sum += samples[i] * samples[i];
  leveler.update(sum / samples.length, TICK_MS);
  // glide slowly so the change is never heard as a jump
  bus.level.gain.setTargetAtTime(Math.pow(10, leveler.gainDb() / 20), bus.context.currentTime, 1.2);
}

function startLeveler(): void {
  if (levelTimer) return;
  levelTimer = setInterval(tick, TICK_MS);
}
