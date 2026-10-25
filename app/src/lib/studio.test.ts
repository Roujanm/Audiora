import { test } from 'node:test';
import assert from 'node:assert/strict';
import { formatTime, nextTrackId, projectEnd, rulerStep, splitClip, type Project } from './studio.ts';

const clip = { take: 'a.wav', lane: 0, start: 100, offset: 10, length: 50 };

test('split keeps the audio continuous', () => {
  const r = splitClip(clip, 120)!;
  assert.deepEqual(r[0], { ...clip, length: 20 });
  assert.deepEqual(r[1], { take: 'a.wav', lane: 0, start: 120, offset: 30, length: 30 });
  assert.equal(splitClip(clip, 100), null);
  assert.equal(splitClip(clip, 150), null);
});

test('project end, ids and time formatting', () => {
  const p: Project = {
    version: 1,
    name: 'x',
    tracks: [
      { id: 't1', name: 'a', source: 'post/4', color: '', gain_db: 0, mute: false, solo: false, armed: false, clips: [clip] },
      { id: 't3', name: 'b', source: 'post/0', color: '', gain_db: 0, mute: false, solo: false, armed: false, clips: [] },
    ],
  };
  assert.equal(projectEnd(p), 150);
  assert.equal(nextTrackId(p), 't4');
  assert.equal(formatTime(48000 * 65.32, 48000), '1:05.32');
  assert.equal(formatTime(0, 48000), '0:00.00');
});

test('ruler labels stay readable at every zoom', () => {
  assert.equal(rulerStep(100), 1);
  assert.equal(rulerStep(10), 10);
  assert.equal(rulerStep(1000), 0.1);
});
