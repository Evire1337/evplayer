// "Without vocals" for any song, at once, with almost no work for the device.
//
// A singer is almost always in the middle of the recording: the same in the left and the
// right channel. What is in the middle cancels out when one channel is taken from the other,
// and what is spread to the sides (guitars, keys, the width of the drums) stays. The middle
// also holds the bass and the kick, so the low end of the middle is added back. Nothing is
// analysed or learned: five small filters. It is rough: a voice with its own wide reverb
// leaves a trace, a mono recording cannot be treated at all, and the sound turns a little
// thinner (the result is the same in both ears).
//
// The stage sits before the pitch shifter, the reverb and the equalizer, so all of them work on
// the result as they do on any song.

/** below this the middle is kept: the bass and the kick live there, a voice hardly does */
export const BASS_KEEP_HZ = 150;
/** the sides alone are quieter than the whole song; this brings them back up a little */
const SIDE_GAIN = 0.7;

export interface VocalCutStage {
  input: AudioNode;
  output: AudioNode;
  /** 0 leaves the song as it is, 1 takes the middle out completely */
  setAmount: (amount: number) => void;
}

export function createVocalCut(ctx: BaseAudioContext): VocalCutStage {
  const input = ctx.createGain();
  input.channelCount = 2;
  input.channelCountMode = "explicit";
  input.channelInterpretation = "speakers";

  const split = ctx.createChannelSplitter(2);
  input.connect(split);

  // the sides: left minus right
  const side = ctx.createGain();
  side.gain.value = SIDE_GAIN;
  const invert = ctx.createGain();
  invert.gain.value = -1;
  split.connect(side, 0);
  split.connect(invert, 1);
  invert.connect(side);

  // the low end of the middle: (left + right) / 2, without what is above the bass
  const middle = ctx.createGain();
  middle.gain.value = 0.5;
  split.connect(middle, 0);
  split.connect(middle, 1);
  const bass = ctx.createBiquadFilter();
  bass.type = "lowpass";
  bass.frequency.value = BASS_KEEP_HZ;
  bass.Q.value = 0.707;
  middle.connect(bass);

  // both ears get the same: a one-channel result is spread to two here, so everything after
  // this (which expects stereo) gets stereo
  const wet = ctx.createGain();
  wet.channelCount = 2;
  wet.channelCountMode = "explicit";
  wet.channelInterpretation = "speakers";
  side.connect(wet);
  bass.connect(wet);

  const dry = ctx.createGain();
  dry.channelCount = 2;
  dry.channelCountMode = "explicit";
  dry.channelInterpretation = "speakers";
  input.connect(dry);

  const output = ctx.createGain();
  output.channelCount = 2;
  output.channelCountMode = "explicit";
  output.channelInterpretation = "speakers";
  dry.connect(output);
  wet.connect(output);

  const setAmount = (amount: number) => {
    const a = Math.min(1, Math.max(0, amount));
    dry.gain.value = 1 - a;
    wet.gain.value = a;
  };
  setAmount(1);
  return { input, output, setAmount };
}
