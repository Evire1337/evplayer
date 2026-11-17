// The sound mixer: how loud the voice, the bass and the sides of a recording are, for every
// song, at once, with almost no work for the device.
//
// Nothing is analysed or learned: a few small filters. The recording is split into its middle
// (what is the same in the left and the right channel) and its sides (the difference). A singer
// sits in the middle of most recordings and lives mostly between about 200 and 4000 Hz. So:
//  - voice: that band of the middle is made louder or quieter (0 = "without vocals");
//  - bass: what is below 150 Hz of the middle;
//  - sides: how loud the sides are, "the instruments" (0 = mono, 1 = as recorded).
// The middle and the sides are put back together after that. "Solo" plays one of the three
// alone. It is rough: a voice with its own wide reverb leaves a trace after being taken out, a
// mono recording has no sides to tell the voice from, and the snare and a guitar in the middle
// lose a part of their body with the voice. At 100% everywhere nothing changes at all.
//
// The stage sits before the pitch shifter, the reverb and the equalizer, so all of them work on
// the result as they do on any song.

export type MixKey = "voice" | "bass" | "width";
export const MIX_KEYS: readonly MixKey[] = ["voice", "bass", "width"];

/** the voice band is cut or lifted by three bells, around these */
export const VOICE_BELLS_HZ = [350, 900, 2300] as const;
export const BELL_Q = 1.0;
/** the voice band alone (solo), and "the bass" */
export const VOICE_LOW_HZ = 200;
export const VOICE_HIGH_HZ = 4000;
export const BASS_HZ = 150;
/** how far a level can go (1 = as recorded) */
export const MAX_LEVEL = 1.5;
/** the deepest cut, in dB: a level of 0 is not silence on a filter but this far down */
export const DEEPEST_DB = -40;
/** the bells add up where they overlap: each takes this part of the wanted change */
const BELL_SHARE = 0.55;

export interface MixerLevels {
  voice: number;
  bass: number;
  width: number;
  /** the one that plays alone, or none */
  solo: MixKey | null;
}

export const FLAT: MixerLevels = { voice: 1, bass: 1, width: 1, solo: null };

export const isFlat = (l: MixerLevels): boolean => l.voice === 1 && l.bass === 1 && l.width === 1 && l.solo === null;

const clamp = (v: number): number => (Number.isFinite(v) ? Math.min(MAX_LEVEL, Math.max(0, v)) : 1);

/** a level (1 = as recorded) as a change in dB; 0 goes to the deepest cut */
export function levelToDb(level: number): number {
  const l = clamp(level);
  return l <= 0 ? DEEPEST_DB : Math.max(DEEPEST_DB, 20 * Math.log10(l));
}

/** the levels the sound gets from the faders, the muted ones and the solo */
export function effectiveLevels(faders: Record<MixKey, number>, muted: readonly MixKey[], solo: MixKey | null): MixerLevels {
  const level = (k: MixKey) => (muted.includes(k) ? 0 : faders[k]);
  return { voice: level("voice"), bass: level("bass"), width: level("width"), solo };
}

export interface SoundMixerStage {
  input: AudioNode;
  output: AudioNode;
  setLevels: (levels: MixerLevels) => void;
}

export function createSoundMixer(ctx: BaseAudioContext): SoundMixerStage {
  const mono = (gain = 1): GainNode => {
    const g = ctx.createGain();
    g.channelCount = 1;
    g.channelCountMode = "explicit";
    g.gain.value = gain;
    return g;
  };
  const biquad = (type: BiquadFilterType, hz: number, q = Math.SQRT1_2): BiquadFilterNode => {
    const f = ctx.createBiquadFilter();
    f.type = type;
    f.frequency.value = hz;
    f.Q.value = q;
    return f;
  };
  /** filters in a row: a steeper edge */
  const chain = (from: AudioNode, ...filters: AudioNode[]): AudioNode => {
    let tail = from;
    for (const f of filters) {
      tail.connect(f);
      tail = f;
    }
    return tail;
  };

  const input = ctx.createGain();
  input.channelCount = 2;
  input.channelCountMode = "explicit";
  input.channelInterpretation = "speakers";
  const split = ctx.createChannelSplitter(2);
  input.connect(split);

  // the middle: (left + right) / 2; the sides: (left - right) / 2
  const middle = mono(0.5);
  split.connect(middle, 0);
  split.connect(middle, 1);
  const side = mono();
  const sideLeft = mono(0.5);
  const sideRight = mono(-0.5);
  split.connect(sideLeft, 0);
  split.connect(sideRight, 1);
  sideLeft.connect(side);
  sideRight.connect(side);

  // the output of both ways
  const left = mono(0);
  const right = mono(0);

  // the whole song: the middle through the voice bells and the bass shelf, the sides on their own
  const bells = VOICE_BELLS_HZ.map((hz) => biquad("peaking", hz, BELL_Q));
  const shelf = biquad("lowshelf", BASS_HZ);
  chain(middle, ...bells, shelf);
  const gateLeft = mono();
  const gateRight = mono();
  const sideToLeft = mono();
  const sideToRight = mono(-1);
  shelf.connect(gateLeft);
  shelf.connect(gateRight);
  side.connect(sideToLeft);
  side.connect(sideToRight);
  sideToLeft.connect(gateLeft);
  sideToRight.connect(gateRight);
  gateLeft.connect(left);
  gateRight.connect(right);

  // solo: one of the three alone, cut out cleanly (nothing is subtracted, so nothing leaks)
  const soloVoice = mono(0);
  const soloBass = mono(0);
  const soloSideLeft = mono(0);
  const soloSideRight = mono(0);
  chain(middle, biquad("highpass", VOICE_LOW_HZ), biquad("highpass", VOICE_LOW_HZ), biquad("lowpass", VOICE_HIGH_HZ), biquad("lowpass", VOICE_HIGH_HZ)).connect(soloVoice);
  chain(middle, biquad("lowpass", BASS_HZ), biquad("lowpass", BASS_HZ)).connect(soloBass);
  side.connect(soloSideLeft);
  side.connect(soloSideRight);
  for (const g of [soloVoice, soloBass]) {
    g.connect(left);
    g.connect(right);
  }
  soloSideLeft.connect(left);
  soloSideRight.connect(right);

  const merge = ctx.createChannelMerger(2);
  left.gain.value = 1;
  right.gain.value = 1;
  left.connect(merge, 0, 0);
  right.connect(merge, 0, 1);

  const setLevels = (levels: MixerLevels) => {
    const solo = levels.solo;
    // a solo silences the whole song and opens just its own way
    gateLeft.gain.value = gateRight.gain.value = solo ? 0 : 1;
    soloVoice.gain.value = solo === "voice" ? clamp(levels.voice) : 0;
    soloBass.gain.value = solo === "bass" ? clamp(levels.bass) : 0;
    const sideGain = solo === "width" ? clamp(levels.width) : 0;
    soloSideLeft.gain.value = sideGain;
    soloSideRight.gain.value = -sideGain;

    const bell = levelToDb(levels.voice) * BELL_SHARE;
    for (const f of bells) f.gain.value = bell;
    shelf.gain.value = levelToDb(levels.bass);
    const w = clamp(levels.width);
    sideToLeft.gain.value = w;
    sideToRight.gain.value = -w;
  };
  return { input, output: merge, setLevels };
}
