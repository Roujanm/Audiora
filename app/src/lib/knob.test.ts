import { test } from 'node:test';
import assert from 'node:assert/strict';
import { toFrac, fromFrac, snap, dragFrac, fracToAngle } from './knob.ts';
import { gainReductionDb, outputDb, gateGainDb } from './dynamics.ts';

test('linear and logarithmic mapping round-trip', () => {
  for (const [min, max, scale] of [[-60, 12, 'lin'], [20, 20000, 'log'], [0.1, 18, 'log']] as const) {
    for (const f of [0, 0.25, 0.5, 0.9, 1]) {
      assert.ok(Math.abs(toFrac(fromFrac(f, min, max, scale), min, max, scale) - f) < 1e-9);
    }
  }
  assert.ok(Math.abs(fromFrac(0.5, 20, 20000, 'log') - 632.455) < 0.01, 'geometric midpoint');
});

test('values outside the range are clamped, not extrapolated', () => {
  assert.equal(toFrac(500, 0, 100), 1);
  assert.equal(toFrac(-5, 0, 100), 0);
  assert.equal(toFrac(1, 20, 20000, 'log'), 0);
});

test('snap respects step, range and float noise', () => {
  assert.equal(snap(0.3000000004, 0.1, 0, 1), 0.3);
  assert.equal(snap(7.4, 5, 0, 20), 5);
  assert.equal(snap(99, 1, 0, 10), 10);
  assert.equal(snap(-3.14159, 0.5, -24, 24), -3);
});

test('dragging up raises the value and fine mode is five times slower', () => {
  assert.ok(dragFrac(0.5, -20) > 0.5);
  assert.ok(dragFrac(0.5, 20) < 0.5);
  const coarse = dragFrac(0.5, -20) - 0.5;
  const fine = dragFrac(0.5, -20, true) - 0.5;
  assert.ok(Math.abs(coarse / fine - 5) < 1e-9);
  assert.equal(dragFrac(0.99, -1000), 1);
});

test('knob sweep is 270 degrees centred on straight up', () => {
  assert.equal(fracToAngle(0), -135);
  assert.equal(fracToAngle(0.5), 0);
  assert.equal(fracToAngle(1), 135);
});

// Same expectations as the Rust compressor test: 4:1 at -20 dB threshold, -6 dB in => 10.5 dB reduction.
test('compressor curve matches the engine', () => {
  const c = { thresholdDb: -20, ratio: 4, kneeDb: 0 };
  assert.ok(Math.abs(gainReductionDb(-6, c) - 10.5) < 1e-9);
  assert.equal(gainReductionDb(-30, c), 0);
  assert.ok(Math.abs(outputDb(-6, c) - -16.5) < 1e-9);
  assert.ok(Math.abs(outputDb(-6, c, 6) - -10.5) < 1e-9);
});

test('soft knee blends smoothly through the threshold', () => {
  const c = { thresholdDb: -20, ratio: 4, kneeDb: 10 };
  assert.equal(gainReductionDb(-26, c), 0);
  const mid = gainReductionDb(-20, c);
  assert.ok(mid > 0 && mid < 10.5 * 0.2, `knee centre ${mid}`);
  let prev = -1;
  for (let db = -30; db <= 0; db += 0.5) {
    const g = gainReductionDb(db, c);
    assert.ok(g >= prev - 1e-12, 'monotonic');
    prev = g;
  }
});

test('gate attenuates below threshold only', () => {
  assert.equal(gateGainDb(-30, -50, 60), 0);
  assert.equal(gateGainDb(-70, -50, 60), -60);
});
