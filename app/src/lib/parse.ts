// Parsing of values typed into read-outs: "-6", "−6.5 dB", "2.5k", "3,5", "-inf".

/** Returns the number, or null when the text is not a value. Accepts a Unicode minus, a decimal comma,
 * a trailing "k" (x1000) and ignores a unit suffix. "-inf", "−∞" and "off" mean minus infinity. */
export function parseNumber(text: string): number | null {
  const t = text.trim().toLowerCase().replace(/−/g, '-').replace(',', '.');
  if (/^(-\s*inf(inity)?|-\s*∞|off|mute)$/.test(t)) return Number.NEGATIVE_INFINITY;
  const m = /^([+-]?\d*\.?\d+)\s*(k)?(?:\s*[a-z%:]+.*)?$/.exec(t);
  if (!m) return null;
  const n = Number(m[1]) * (m[2] ? 1000 : 1);
  return Number.isFinite(n) ? n : null;
}

/** "L30" / "R40" / "C" / "-0.3" -> balance in -1..1. */
export function parseBalance(text: string): number | null {
  const t = text.trim().toLowerCase();
  if (t === 'c' || t === 'center' || t === 'centre') return 0;
  const m = /^([lr])\s*(\d{1,3})%?$/.exec(t);
  if (m) return (m[1] === 'l' ? -1 : 1) * Math.min(Number(m[2]), 100) / 100;
  const n = parseNumber(t);
  return n === null || !Number.isFinite(n) ? null : Math.min(Math.max(n, -1), 1);
}
