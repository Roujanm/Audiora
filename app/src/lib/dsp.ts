// Mirrors crates/audiora-dsp/src/biquad.rs (RBJ cookbook) so the EQ curve the
// user sees is the curve the engine applies.

export type FilterKind = 'lowshelf' | 'peak' | 'peak2' | 'highshelf';

/** Band layout is fixed by the engine: 0 low shelf, 1 peak, 2 peak, 3 high shelf. */
export const BAND_KINDS: FilterKind[] = ['lowshelf', 'peak', 'peak2', 'highshelf'];

export interface Band {
  freq: number;
  gainDb: number;
  q: number;
}

interface Coeffs {
  b0: number;
  b1: number;
  b2: number;
  a1: number;
  a2: number;
}

export function coeffs(kind: FilterKind, fs: number, freq: number, q: number, gainDb: number): Coeffs {
  const f = Math.min(Math.max(freq, 10), fs * 0.49);
  const qq = Math.max(q, 0.05);
  const a = Math.pow(10, gainDb / 40);
  const w0 = (2 * Math.PI * f) / fs;
  const cos = Math.cos(w0);
  const alpha = Math.sin(w0) / (2 * qq);
  const s2a = 2 * Math.sqrt(a) * alpha;
  let b0: number, b1: number, b2: number, a0: number, a1: number, a2: number;
  if (kind === 'lowshelf') {
    b0 = a * (a + 1 - (a - 1) * cos + s2a);
    b1 = 2 * a * (a - 1 - (a + 1) * cos);
    b2 = a * (a + 1 - (a - 1) * cos - s2a);
    a0 = a + 1 + (a - 1) * cos + s2a;
    a1 = -2 * (a - 1 + (a + 1) * cos);
    a2 = a + 1 + (a - 1) * cos - s2a;
  } else if (kind === 'highshelf') {
    b0 = a * (a + 1 + (a - 1) * cos + s2a);
    b1 = -2 * a * (a - 1 + (a + 1) * cos);
    b2 = a * (a + 1 + (a - 1) * cos - s2a);
    a0 = a + 1 - (a - 1) * cos + s2a;
    a1 = 2 * (a - 1 - (a + 1) * cos);
    a2 = a + 1 - (a - 1) * cos - s2a;
  } else {
    b0 = 1 + alpha * a;
    b1 = -2 * cos;
    b2 = 1 - alpha * a;
    a0 = 1 + alpha / a;
    a1 = -2 * cos;
    a2 = 1 - alpha / a;
  }
  return { b0: b0 / a0, b1: b1 / a0, b2: b2 / a0, a1: a1 / a0, a2: a2 / a0 };
}

export function magnitudeDb(c: Coeffs, fs: number, freq: number): number {
  const w = (2 * Math.PI * freq) / fs;
  const c1 = Math.cos(-w), s1 = Math.sin(-w);
  const c2 = Math.cos(-2 * w), s2 = Math.sin(-2 * w);
  const nr = c.b0 + c.b1 * c1 + c.b2 * c2;
  const ni = c.b1 * s1 + c.b2 * s2;
  const dr = 1 + c.a1 * c1 + c.a2 * c2;
  const di = c.a1 * s1 + c.a2 * s2;
  return 10 * Math.log10((nr * nr + ni * ni) / (dr * dr + di * di));
}

/** Combined response of all bands at `freq` (bands at ~0 dB are skipped, like the engine does). */
export function eqResponseDb(bands: Band[], fs: number, freq: number): number {
  let total = 0;
  bands.forEach((b, i) => {
    if (Math.abs(b.gainDb) < 0.01) return;
    total += magnitudeDb(coeffs(BAND_KINDS[i], fs, b.freq, b.q, b.gainDb), fs, freq);
  });
  return total;
}

export const MIN_DB = -120;

export function linToDb(lin: number): number {
  return lin <= 1e-6 ? MIN_DB : 20 * Math.log10(lin);
}

export function dbToLin(db: number): number {
  return db <= MIN_DB ? 0 : Math.pow(10, db / 20);
}

export function formatDb(db: number, digits = 1): string {
  return db <= -100 ? '−∞' : (db > 0 ? '+' : '') + db.toFixed(digits).replace('-', '−');
}

/** Log-frequency position 0..1 for 20 Hz .. 20 kHz. */
export function freqToX(freq: number): number {
  return (Math.log10(freq) - Math.log10(20)) / (Math.log10(20000) - Math.log10(20));
}

export function xToFreq(x: number): number {
  return Math.pow(10, Math.log10(20) + x * (Math.log10(20000) - Math.log10(20)));
}
