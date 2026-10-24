// Value <-> position maths shared by the rotary knob and the response graphs.

export type Scale = 'lin' | 'log';

const clamp01 = (x: number) => Math.min(Math.max(x, 0), 1);

export function toFrac(value: number, min: number, max: number, scale: Scale = 'lin'): number {
  if (scale === 'log') {
    const v = Math.min(Math.max(value, min), max);
    return clamp01((Math.log(v) - Math.log(min)) / (Math.log(max) - Math.log(min)));
  }
  return clamp01((value - min) / (max - min));
}

export function fromFrac(frac: number, min: number, max: number, scale: Scale = 'lin'): number {
  const f = clamp01(frac);
  if (scale === 'log') return Math.exp(Math.log(min) + f * (Math.log(max) - Math.log(min)));
  return min + f * (max - min);
}

/** Rounds to the control's step (and to a sensible number of decimals). */
export function snap(value: number, step: number, min: number, max: number): number {
  if (!(step > 0)) return Math.min(Math.max(value, min), max);
  const snapped = Math.round((value - min) / step) * step + min;
  const decimals = Math.max(0, Math.ceil(-Math.log10(step)));
  return Math.min(Math.max(Number(snapped.toFixed(decimals)), min), max);
}

/** A vertical drag of `dy` pixels (up is negative) moves the knob by `dy / travel` of its range. */
export function dragFrac(startFrac: number, dy: number, fine = false, travel = 200): number {
  return clamp01(startFrac - dy / (fine ? travel * 5 : travel));
}

/** Angle in degrees for the 270-degree sweep (-135 .. +135), 0 = straight up. */
export function fracToAngle(frac: number): number {
  return -135 + clamp01(frac) * 270;
}
