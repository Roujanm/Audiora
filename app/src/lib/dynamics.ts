// Static transfer curve of the engine's compressor (mirrors crates/audiora-dsp/src/dynamics.rs),
// used to draw the curve and to place the live level marker.

export interface CompShape {
  thresholdDb: number;
  ratio: number;
  kneeDb: number;
}

/** Gain reduction in dB (>= 0) the compressor applies to a steady input level. */
export function gainReductionDb(inDb: number, c: CompShape): number {
  const ratio = Math.max(c.ratio, 1);
  const slope = 1 - 1 / ratio;
  const over = inDb - c.thresholdDb;
  const half = Math.max(c.kneeDb, 0) / 2;
  if (half > 0 && Math.abs(over) <= half) {
    const x = over + half;
    return (slope * x * x) / (4 * half);
  }
  return over > 0 ? slope * over : 0;
}

export function outputDb(inDb: number, c: CompShape, makeupDb = 0): number {
  return inDb - gainReductionDb(inDb, c) + makeupDb;
}

/** Gate: static attenuation below the threshold, limited to `rangeDb`. */
export function gateGainDb(inDb: number, thresholdDb: number, rangeDb: number): number {
  return inDb >= thresholdDb ? 0 : -Math.abs(rangeDb);
}
