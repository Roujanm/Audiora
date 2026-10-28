import type { IconName } from '../components/Icon.svelte';
import type { ChannelKey } from './protocol.ts';

export interface ChannelMeta {
  key: ChannelKey;
  name: string;
  icon: IconName;
  color: string;
  /** One-line purpose, shown on the channel's page. */
  blurb: string;
}

/** Display order of the mixer, left to right after the master. */
export const CHANNELS: ChannelMeta[] = [
  { key: 'game', name: 'Game', icon: 'gamepad', color: '#4d9bff', blurb: 'Games and anything that needs positional clarity.' },
  { key: 'chat', name: 'Chat', icon: 'chat', color: '#35e0a1', blurb: 'Discord, Teams, TeamSpeak — the voices of the people you talk to.' },
  { key: 'media', name: 'Media', icon: 'play', color: '#ff7a9c', blurb: 'Browsers, video and streaming.' },
  { key: 'aux', name: 'Aux', icon: 'aux', color: '#a58bff', blurb: 'Anything else you want on its own fader.' },
  { key: 'music', name: 'Music', icon: 'music', color: '#ffb347', blurb: 'Spotify, Apple Music, YouTube Music — your playlists on their own fader.' },
  { key: 'mic', name: 'Mic', icon: 'mic', color: '#2ed3e6', blurb: 'Your microphone, cleaned up before it reaches anyone.' },
];

export const APP_CHANNEL_KEYS: ChannelKey[] = ['game', 'chat', 'media', 'aux', 'music'];

export function channelMeta(key: string | null | undefined): ChannelMeta | undefined {
  return CHANNELS.find((c) => c.key === key);
}

/** A stable, pleasant colour for an application avatar, derived from its name. */
export function appColor(name: string): string {
  let h = 0;
  for (const ch of name.toLowerCase()) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
  return `hsl(${h % 360} 62% 58%)`;
}

/** "callofduty" -> "Callofduty", "Ready Or Not" stays; keeps executable names readable. */
export function appLabel(name: string): string {
  const spaced = name.replace(/[_-]+/g, ' ').trim();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
}

/** Smart Volume loudness presets: target level in dBFS. */
export const SMART_LEVELS = [
  { id: 'quiet', label: 'Quiet', db: -30 },
  { id: 'balanced', label: 'Balanced', db: -24 },
  { id: 'loud', label: 'Loud', db: -18 },
] as const;

export function smartLevelOf(db: number): string {
  return SMART_LEVELS.reduce((best, l) => (Math.abs(l.db - db) < Math.abs(best.db - db) ? l : best)).id;
}

/** Basic equalizer: three sliders that move pairs of bands, like a hi-fi tone control. */
export const BASIC_EQ = [
  { id: 'bass', label: 'Bass', bands: [0, 1], weights: [1, 1] },
  { id: 'voice', label: 'Voice', bands: [2, 3], weights: [0.8, 1] },
  { id: 'treble', label: 'Treble', bands: [4, 5], weights: [1, 1] },
] as const;
