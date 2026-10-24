import { linToDb } from './dsp.ts';

export interface MeterState {
  /** Displayed level in dBFS: instant attack, steady decay. */
  db: number;
  /** Peak-hold marker in dBFS. */
  hold: number;
  /** Timestamp (ms) when the hold marker was last raised. */
  holdAt: number;
}

export const DECAY_DB_PER_S = 26;
export const HOLD_MS = 1500;

/**
 * Advances one meter by a new linear peak reading.
 * `now` and `elapsedMs` are milliseconds (time since the previous reading).
 */
export function stepMeter(
  prev: MeterState | undefined,
  lin: number,
  now: number,
  elapsedMs: number,
): MeterState {
  const target = linToDb(lin);
  if (!prev) return { db: target, hold: target, holdAt: now };

  const fall = (DECAY_DB_PER_S * elapsedMs) / 1000;
  const db = Math.max(target, prev.db - fall);
  let { hold, holdAt } = prev;
  if (db >= hold) {
    hold = db;
    holdAt = now;
  } else if (now - holdAt > HOLD_MS) {
    hold = Math.max(db, hold - fall);
  }
  return { db, hold, holdAt };
}

export const SPECTRUM_DECAY_DB_PER_S = 34;
export const SPECTRUM_FLOOR_DB = -120;

/** Fast attack, steady decay per band, like the level meters. */
export function stepSpectrum(prev: number[] | null, next: number[], elapsedMs: number): number[] {
  if (!prev || prev.length !== next.length) return next.slice();
  const fall = (SPECTRUM_DECAY_DB_PER_S * Math.min(Math.max(elapsedMs, 0), 500)) / 1000;
  return next.map((v, i) => Math.max(v, prev[i] - fall, SPECTRUM_FLOOR_DB));
}
