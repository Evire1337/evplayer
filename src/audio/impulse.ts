/** a synthetic room: stereo noise with an exponential tail, gently darkened */
export function makeImpulse(ctx: BaseAudioContext, seconds = 2.6, decay = 3.2): AudioBuffer {
  const length = Math.floor(ctx.sampleRate * seconds);
  const buffer = ctx.createBuffer(2, length, ctx.sampleRate);
  for (let c = 0; c < 2; c++) {
    const data = buffer.getChannelData(c);
    let last = 0;
    for (let i = 0; i < length; i++) {
      const t = i / length;
      const noise = Math.random() * 2 - 1;
      // one-pole low-pass: the tail loses its highs as it fades, like a real hall
      last = last + (noise - last) * (0.55 - 0.4 * t);
      data[i] = last * Math.pow(1 - t, decay);
    }
  }
  return buffer;
}
