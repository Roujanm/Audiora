import type { NodeInfo } from './protocol.ts';

/** One hue per channel so a strip is recognisable at a glance across every view. */
const HUES = ['#4d9bff', '#35e0a1', '#ff7a9c', '#a58bff', '#2ed3e6', '#ffd24d'];

export function stripColor(node: Pick<NodeInfo, 'kind'> | undefined, index: number): string {
  if (node?.kind === 'tone') return '#ffb347';
  return HUES[index % HUES.length];
}

export const ACCENTS = [
  { id: 'mono', label: 'Audiora mono', swatch: '#e6e9ef' },
  { id: 'azure', label: 'Azure', swatch: '#4d9bff' },
  { id: 'violet', label: 'Violet', swatch: '#9a86ff' },
  { id: 'emerald', label: 'Emerald', swatch: '#34dba0' },
  { id: 'amber', label: 'Amber', swatch: '#ffb23e' },
  { id: 'rose', label: 'Rose', swatch: '#ff6b8f' },
] as const;

export type AccentId = (typeof ACCENTS)[number]['id'];

export function rateLabel(rate: number | null | undefined): string {
  if (!rate) return '';
  return `${rate % 1000 === 0 ? rate / 1000 : (rate / 1000).toFixed(1)} kHz`;
}

/** Period in milliseconds, e.g. "10 ms". */
export function periodLabel(rate: number | null | undefined, frames: number | null | undefined): string {
  if (!rate || !frames) return '';
  const ms = (frames / rate) * 1000;
  return `${ms < 10 ? ms.toFixed(1) : Math.round(ms)} ms`;
}

/** Strips a trailing "(driver name)" from a Windows device name: "Headphones (JBL Quantum 360)" stays, "Mic (Realtek(R) Audio)" -> "Mic". */
export function shortName(name: string): string {
  const m = /^(.*?)\s*\(.*\)\s*$/.exec(name);
  return (m?.[1] || name).trim();
}
