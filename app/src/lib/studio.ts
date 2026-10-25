// Studio project model and timeline maths (pure; covered by tests).

export interface Clip {
  /** Take file inside the project's `takes` folder. */
  take: string;
  /** Stereo lane of that take. */
  lane: number;
  /** Timeline position, frames. */
  start: number;
  /** Frames skipped at the beginning of the take lane. */
  offset: number;
  length: number;
}

export interface Track {
  id: string;
  name: string;
  source: string;
  color: string;
  gain_db: number;
  mute: boolean;
  solo: boolean;
  armed: boolean;
  clips: Clip[];
}

export interface Project {
  version: number;
  name: string;
  tracks: Track[];
}

export interface PeakData {
  bin: number;
  frames: number;
  lanes: number[][];
}

/** What a track can record, with the meter that shows its level. */
export const SOURCES = [
  { id: 'post/4', label: 'Mic · processed', meter: 'strip/4' },
  { id: 'strip/4', label: 'Mic · raw', meter: 'strip/4' },
  { id: 'post/0', label: 'Game', meter: 'strip/0' },
  { id: 'post/1', label: 'Chat', meter: 'strip/1' },
  { id: 'post/2', label: 'Media', meter: 'strip/2' },
  { id: 'post/3', label: 'Aux', meter: 'strip/3' },
  { id: 'bus/0', label: 'Master mix', meter: 'bus/0' },
] as const;

export const TRACK_COLORS = ['#2ed3e6', '#4d9bff', '#35e0a1', '#ff7a9c', '#a58bff', '#ffb347', '#ffd24d'];

export function projectEnd(p: Project): number {
  let end = 0;
  for (const t of p.tracks) for (const c of t.clips) end = Math.max(end, c.start + c.length);
  return end;
}

/** "1:05.32" */
export function formatTime(frames: number, rate: number): string {
  const s = Math.max(0, frames / rate);
  const m = Math.floor(s / 60);
  const rest = s - m * 60;
  return `${m}:${rest.toFixed(2).padStart(5, '0')}`;
}

/** Splits `clip` at timeline frame `at`; returns the two halves, or null when `at` is not inside. */
export function splitClip(clip: Clip, at: number): [Clip, Clip] | null {
  const cut = Math.round(at - clip.start);
  if (cut <= 0 || cut >= clip.length) return null;
  return [
    { ...clip, length: cut },
    { ...clip, start: clip.start + cut, offset: clip.offset + cut, length: clip.length - cut },
  ];
}

/** A friendly unused track id. */
export function nextTrackId(p: Project): string {
  let n = p.tracks.length + 1;
  while (p.tracks.some((t) => t.id === `t${n}`)) n++;
  return `t${n}`;
}

/** Seconds between ruler labels for a zoom level, so labels stay about 80 px apart. */
export function rulerStep(pxPerSecond: number): number {
  const steps = [0.1, 0.25, 0.5, 1, 2, 5, 10, 15, 30, 60, 120, 300];
  return steps.find((s) => s * pxPerSecond >= 80) ?? 600;
}
