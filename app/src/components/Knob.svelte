<script lang="ts">
  import { dragFrac, fracToAngle, fromFrac, snap, toFrac, type Scale } from '../lib/knob.ts';

  interface Props {
    label?: string;
    value: number;
    min: number;
    max: number;
    step?: number;
    unit?: string;
    digits?: number;
    /** Value restored on double-click. */
    reset?: number;
    scale?: Scale;
    /** Arc grows from the centre (for pan, trim...). */
    bipolar?: boolean;
    size?: number;
    /** CSS colour of the value arc; defaults to the accent. */
    color?: string;
    disabled?: boolean;
    format?: (v: number) => string;
    onchange: (v: number) => void;
  }
  let {
    label = '',
    value,
    min,
    max,
    step = 1,
    unit = '',
    digits = 1,
    reset,
    scale = 'lin',
    bipolar = false,
    size = 58,
    color = 'var(--accent)',
    disabled = false,
    format,
    onchange,
  }: Props = $props();

  const frac = $derived(toFrac(value, min, max, scale));
  const angle = $derived(fracToAngle(frac));
  const c = $derived(size / 2);
  const r = $derived(size / 2 - 5);

  function pt(deg: number, radius: number) {
    const a = ((deg - 90) * Math.PI) / 180;
    return [c + radius * Math.cos(a), c + radius * Math.sin(a)] as const;
  }

  function arc(from: number, to: number, radius: number) {
    if (Math.abs(to - from) < 0.01) return '';
    const [x0, y0] = pt(from, radius);
    const [x1, y1] = pt(to, radius);
    const large = Math.abs(to - from) > 180 ? 1 : 0;
    return `M${x0.toFixed(2)} ${y0.toFixed(2)}A${radius} ${radius} 0 ${large} ${to > from ? 1 : 0} ${x1.toFixed(2)} ${y1.toFixed(2)}`;
  }

  const trackPath = $derived(arc(-135, 135, r));
  const valuePath = $derived(bipolar ? arc(fracToAngle(0.5), angle, r) : arc(-135, angle, r));
  const [tipX, tipY] = $derived(pt(angle, r - 4));
  const [baseX, baseY] = $derived(pt(angle, r * 0.38));

  const text = $derived(format ? format(value) : `${value.toFixed(digits)}${unit}`);

  let dragging = $state(false);
  let startY = 0;
  let startFrac = 0;

  function commit(f: number) {
    const v = snap(fromFrac(f, min, max, scale), step, min, max);
    if (v !== value) onchange(v);
  }

  function onpointerdown(e: PointerEvent) {
    if (disabled) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragging = true;
    startY = e.clientY;
    startFrac = frac;
  }

  function onpointermove(e: PointerEvent) {
    if (!dragging) return;
    commit(dragFrac(startFrac, e.clientY - startY, e.shiftKey));
  }

  function onkeydown(e: KeyboardEvent) {
    if (disabled) return;
    const big = e.shiftKey ? 10 : 1;
    const dir: Record<string, number> = { ArrowUp: 1, ArrowRight: 1, ArrowDown: -1, ArrowLeft: -1, PageUp: 10, PageDown: -10 };
    if (e.key in dir) {
      e.preventDefault();
      const next = scale === 'log' ? value * Math.pow(1.05, dir[e.key] * big) : value + dir[e.key] * big * step;
      onchange(snap(next, step, min, max));
    } else if (e.key === 'Home') {
      e.preventDefault();
      onchange(min);
    } else if (e.key === 'End') {
      e.preventDefault();
      onchange(max);
    }
  }

  function onwheel(e: WheelEvent) {
    if (disabled) return;
    e.preventDefault();
    commit(frac + (e.deltaY < 0 ? 1 : -1) * (e.shiftKey ? 0.004 : 0.02));
  }
</script>

<div class="knob" class:disabled class:dragging style:--c={color}>
  {#if label}<span class="label">{label}</span>{/if}
  <div
    class="dial"
    role="slider"
    tabindex={disabled ? -1 : 0}
    aria-label={label || 'Value'}
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={value}
    aria-valuetext={text}
    aria-disabled={disabled}
    style:width="{size}px"
    style:height="{size}px"
    {onpointerdown}
    {onpointermove}
    onpointerup={() => (dragging = false)}
    onpointercancel={() => (dragging = false)}
    ondblclick={() => reset !== undefined && !disabled && onchange(reset)}
    {onkeydown}
    {onwheel}
  >
    <svg width={size} height={size} viewBox="0 0 {size} {size}" aria-hidden="true">
      <path d={trackPath} class="track" />
      {#if valuePath}<path d={valuePath} class="value" />{/if}
      <circle cx={c} cy={c} r={r - 7} class="body" />
      <circle cx={c} cy={c} r={r - 7} class="rim" />
      <line x1={baseX} y1={baseY} x2={tipX} y2={tipY} class="needle" />
    </svg>
  </div>
  <output class="num readout">{text}</output>
</div>

<style>
  .knob {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    min-width: 0;
  }

  .label {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
    white-space: nowrap;
  }

  .dial {
    position: relative;
    touch-action: none;
    cursor: ns-resize;
    border-radius: 50%;
  }

  .dial:focus-visible {
    outline-offset: 3px;
    border-radius: 50%;
  }

  svg {
    display: block;
    overflow: visible;
  }

  .track {
    fill: none;
    stroke: var(--bg-4);
    stroke-width: 3.5;
    stroke-linecap: round;
  }

  .value {
    fill: none;
    stroke: var(--c);
    stroke-width: 3.5;
    stroke-linecap: round;
    filter: drop-shadow(0 0 4px color-mix(in oklab, var(--c) 70%, transparent));
  }

  .body {
    fill: var(--bg-3);
    stroke: none;
    filter: drop-shadow(0 3px 4px rgba(0, 0, 0, 0.6));
  }

  .rim {
    fill: none;
    stroke: var(--line-3);
    stroke-width: 1;
  }

  .needle {
    stroke: var(--text);
    stroke-width: 2.4;
    stroke-linecap: round;
    transition: stroke 0.15s;
  }

  .dial:hover .needle,
  .dragging .needle {
    stroke: var(--c);
  }

  .readout {
    font-size: 11.5px;
    color: var(--text-2);
    min-width: 5ch;
    text-align: center;
  }

  .dragging .readout {
    color: var(--text);
  }

  .disabled {
    opacity: 0.38;
  }

  .disabled .dial {
    cursor: default;
  }
</style>
