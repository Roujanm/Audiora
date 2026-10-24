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
