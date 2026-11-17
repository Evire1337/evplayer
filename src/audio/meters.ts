import { BASS_HZ, VOICE_HIGH_HZ, VOICE_LOW_HZ, type MixKey } from "./soundMixer";

// What each part of the song carries right now, for the "Дорожки" page: three small taps on
// the song (before the mixer), each ending in an analyser. They exist only while the page is
// open, so the rest of the time they cost nothing.

export interface BandMeters {
  analysers: Record<MixKey, AnalyserNode>;
  dispose: () => void;
}

const FFT_SIZE = 1024;

export function createMeters(ctx: BaseAudioContext, source: AudioNode): BandMeters {
  const nodes: AudioNode[] = [];
  const keep = <T extends AudioNode>(n: T): T => {
    nodes.push(n);
    return n;
  };
  const mono = (gain: number) => {
    const g = keep(ctx.createGain());
    g.channelCount = 1;
    g.channelCountMode = "explicit";
    g.gain.value = gain;
    return g;
  };
  const filter = (type: BiquadFilterType, hz: number) => {
    const f = keep(ctx.createBiquadFilter());
    f.type = type;
    f.frequency.value = hz;
    return f;
  };
  const analyser = () => {
    const a = keep(ctx.createAnalyser());
    a.fftSize = FFT_SIZE;
    a.smoothingTimeConstant = 0;
    return a;
  };

  const tap = keep(ctx.createGain());
  tap.channelCount = 2;
  tap.channelCountMode = "explicit";
  tap.channelInterpretation = "speakers";
  source.connect(tap);
  const split = keep(ctx.createChannelSplitter(2));
  tap.connect(split);

  const middle = mono(0.5);
  split.connect(middle, 0);
  split.connect(middle, 1);
  const side = mono(1);
  const sideLeft = mono(0.5);
  const sideRight = mono(-0.5);
  split.connect(sideLeft, 0);
  split.connect(sideRight, 1);
  sideLeft.connect(side);
  sideRight.connect(side);

  const voice = analyser();
  middle.connect(filter("highpass", VOICE_LOW_HZ)).connect(filter("lowpass", VOICE_HIGH_HZ)).connect(voice);
  const bass = analyser();
  middle.connect(filter("lowpass", BASS_HZ)).connect(bass);
  const width = analyser();
  side.connect(width);

  return {
    analysers: { voice, bass, width },
    dispose: () => {
      try {
        source.disconnect(tap);
      } catch {
        // already gone
      }
      for (const n of nodes) n.disconnect();
    },
  };
}

/** the loudest sample in an analyser's last moment, 0 to 1 */
export function peakOf(a: AnalyserNode, buffer: Float32Array<ArrayBuffer>): number {
  a.getFloatTimeDomainData(buffer);
  let peak = 0;
  for (let i = 0; i < buffer.length; i++) {
    const v = Math.abs(buffer[i]);
    if (v > peak) peak = v;
  }
  return Math.min(1, peak);
}
