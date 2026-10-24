import { test } from 'node:test';
import assert from 'node:assert/strict';
import { stepMeter, stepSpectrum, DECAY_DB_PER_S, HOLD_MS, SPECTRUM_DECAY_DB_PER_S } from './ballistics.ts';

test('attacks instantly and decays at a fixed dB/s', () => {
  let m = stepMeter(undefined, 1.0, 0, 0);
  assert.equal(m.db, 0);
  m = stepMeter(m, 0, 1000, 1000);
  assert.ok(Math.abs(m.db + DECAY_DB_PER_S) < 1e-9);
  m = stepMeter(m, 0.5, 1033, 33);
  assert.ok(Math.abs(m.db - -6.0206) < 1e-3, 'a louder reading wins immediately');
});

test('peak hold stays, then falls after the hold time', () => {
  let m = stepMeter(undefined, 1.0, 0, 0);
  for (let t = 100; t <= HOLD_MS; t += 100) m = stepMeter(m, 0, t, 100);
  assert.equal(m.hold, 0, 'held during the hold window');
  for (let t = HOLD_MS + 100; t <= HOLD_MS + 2000; t += 100) m = stepMeter(m, 0, t, 100);
  assert.ok(m.hold < -20, `hold should have fallen, is ${m.hold}`);
});

test('silence settles at the floor', () => {
  let m = stepMeter(undefined, 0.5, 0, 0);
  for (let t = 100; t <= 10000; t += 100) m = stepMeter(m, 0, t, 100);
  assert.equal(m.db, -120);
});

test('spectrum bands attack instantly and fall at a fixed rate', () => {
  let s = stepSpectrum(null, [-10, -60, -90], 0);
  assert.deepEqual(s, [-10, -60, -90]);
  s = stepSpectrum(s, [-90, -60, -30], 500);
  assert.ok(Math.abs(s[0] - (-10 - SPECTRUM_DECAY_DB_PER_S / 2)) < 1e-9, 'falls by the decay rate');
  assert.equal(s[1], -60);
  assert.equal(s[2], -30, 'a louder reading wins immediately');
});

test('spectrum smoothing restarts when the band count changes and caps long gaps', () => {
  assert.equal(stepSpectrum([0, 0], [-50, -50, -50], 20).length, 3);
  const s = stepSpectrum([0], [-120], 60_000);
  assert.ok(s[0] > -20, 'a long stall counts as at most 500 ms of decay');
});
