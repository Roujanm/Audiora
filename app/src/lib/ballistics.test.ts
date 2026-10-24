import { test } from 'node:test';
import assert from 'node:assert/strict';
import { stepMeter, DECAY_DB_PER_S, HOLD_MS } from './ballistics.ts';

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
