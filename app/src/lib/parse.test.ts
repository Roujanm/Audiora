import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseNumber, parseBalance } from './parse.ts';

test('plain, signed, unit-suffixed and localised numbers', () => {
  assert.equal(parseNumber('-6'), -6);
  assert.equal(parseNumber('−6.5 dB'), -6.5);
  assert.equal(parseNumber('+3'), 3);
  assert.equal(parseNumber('3,5'), 3.5);
  assert.equal(parseNumber('120 ms'), 120);
  assert.equal(parseNumber('4:1'), 4);
  assert.equal(parseNumber(' .5 '), 0.5);
});

test('k suffix means thousands', () => {
  assert.equal(parseNumber('2.5k'), 2500);
  assert.equal(parseNumber('8 kHz'), 8000);
  assert.equal(parseNumber('1k'), 1000);
});

test('minus infinity spellings', () => {
  for (const t of ['-inf', '−∞', '-INF', 'off']) assert.equal(parseNumber(t), Number.NEGATIVE_INFINITY, t);
});

test('rejects text that is not a value', () => {
  for (const t of ['', 'abc', '--3', '1.2.3', 'dB', 'NaN']) assert.equal(parseNumber(t), null, t);
});

test('balance accepts L/R/C notation and plain fractions', () => {
  assert.equal(parseBalance('C'), 0);
  assert.equal(parseBalance('L30'), -0.3);
  assert.equal(parseBalance('r100'), 1);
  assert.equal(parseBalance('R250'), 1);
  assert.equal(parseBalance('0.25'), 0.25);
  assert.equal(parseBalance('7'), 1);
  assert.equal(parseBalance('left'), null);
});
