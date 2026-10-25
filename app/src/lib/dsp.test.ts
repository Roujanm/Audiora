import { test } from 'node:test';
import assert from 'node:assert/strict';
import { coeffs, magnitudeDb, eqResponseDb, linToDb, dbToLin, freqToX, xToFreq, formatDb } from './dsp.ts';

const FS = 48000;

// The same expectations the Rust tests assert for crates/audiora-dsp biquads.
test('peak filter hits its gain at the centre frequency', () => {
  const c = coeffs('peak', FS, 1000, 1, 6);
  assert.ok(Math.abs(magnitudeDb(c, FS, 1000) - 6) < 0.01);
  assert.ok(Math.abs(magnitudeDb(c, FS, 50)) < 0.3);
});

test('shelves reach their gain far from the corner', () => {
  const ls = coeffs('lowshelf', FS, 200, 0.707, 9);
  assert.ok(Math.abs(magnitudeDb(ls, FS, 20) - 9) < 0.3);
  assert.ok(Math.abs(magnitudeDb(ls, FS, 10000)) < 0.3);
  const hs = coeffs('highshelf', FS, 6000, 0.707, -9);
  assert.ok(Math.abs(magnitudeDb(hs, FS, 20000) + 9) < 0.5);
});

test('combined response sums bands and skips flat ones', () => {
  const flat = { freq: 1000, gainDb: 0, q: 1 };
  assert.equal(eqResponseDb([flat, flat, flat, flat, flat, flat], FS, 1000), 0);
  const bands = [flat, { freq: 1000, gainDb: 6, q: 1 }, { freq: 1000, gainDb: 3, q: 1 }, flat, flat, flat];
  assert.ok(Math.abs(eqResponseDb(bands, FS, 1000) - 9) < 0.05);
});

test('dB helpers', () => {
  assert.equal(linToDb(0), -120);
  assert.ok(Math.abs(dbToLin(-6.0206) - 0.5) < 1e-3);
  assert.equal(formatDb(-120), '−∞');
  assert.equal(formatDb(3), '+3.0');
  assert.equal(formatDb(-2.5), '−2.5');
});

test('frequency axis maps 20 Hz..20 kHz onto 0..1 and back', () => {
  assert.ok(Math.abs(freqToX(20)) < 1e-9 && Math.abs(freqToX(20000) - 1) < 1e-9);
  for (const f of [50, 440, 1000, 8000]) assert.ok(Math.abs(xToFreq(freqToX(f)) - f) < 1e-6);
});
