/** semitones -> playback ratio, and back */
export const semitonesToRatio = (st: number) => Math.pow(2, st / 12);
export const ratioToSemitones = (ratio: number) => 12 * Math.log2(ratio);
