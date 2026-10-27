// Pitch shifter (AudioWorklet). Two read heads move through a short ring buffer at
// `ratio` times the write speed; each head restarts when it drifts a window away, and
// the two are cross-faded with a raised-cosine envelope so the restarts are not heard.
// ratio > 1 raises the pitch, < 1 lowers it; tempo is untouched.

const WINDOW = 2048; // ~43 ms at 48 kHz: short enough to stay tight, long enough for bass
const BUFFER = 8192;

class PitchShifter extends AudioWorkletProcessor {
  static get parameterDescriptors() {
    return [{ name: "ratio", defaultValue: 1, minValue: 0.25, maxValue: 4, automationRate: "k-rate" }];
  }

  constructor() {
    super();
    this.buffers = [];
    this.write = 0;
    this.phase = 0; // 0..1 position inside the window for head A; head B is half a window later
  }

  channel(i) {
    if (!this.buffers[i]) this.buffers[i] = new Float32Array(BUFFER);
    return this.buffers[i];
  }

  read(buf, pos) {
    const i = Math.floor(pos);
    const frac = pos - i;
    const a = buf[((i % BUFFER) + BUFFER) % BUFFER];
    const b = buf[(((i + 1) % BUFFER) + BUFFER) % BUFFER];
    return a + (b - a) * frac;
  }

  process(inputs, outputs, parameters) {
    const input = inputs[0];
    const output = outputs[0];
    if (!input || input.length === 0) return true;
    const ratio = parameters.ratio[0];
    const frames = input[0].length;
    const step = (1 - ratio) / WINDOW;

    for (let c = 0; c < output.length; c++) {
      const src = input[c] || input[0];
      const buf = this.channel(c);
      let phase = this.phase;
      let write = this.write;
      for (let n = 0; n < frames; n++) {
        buf[write % BUFFER] = src[n];
        if (ratio === 1) {
          output[c][n] = src[n];
        } else {
          const phaseB = (phase + 0.5) % 1;
          // a head lags the writer by phase * WINDOW samples; that lag changes with the ratio
          const posA = write - phase * WINDOW - 1;
          const posB = write - phaseB * WINDOW - 1;
          const gainA = 0.5 - 0.5 * Math.cos(2 * Math.PI * phase);
          const gainB = 0.5 - 0.5 * Math.cos(2 * Math.PI * phaseB);
          output[c][n] = this.read(buf, posA) * gainA + this.read(buf, posB) * gainB;
        }
        phase += step;
        if (phase >= 1) phase -= 1;
        if (phase < 0) phase += 1;
        write++;
      }
      if (c === output.length - 1) {
        this.phase = phase;
        this.write = write % (BUFFER * 1024);
      }
    }
    return true;
  }
}

registerProcessor("evplayer-pitch-shifter", PitchShifter);
