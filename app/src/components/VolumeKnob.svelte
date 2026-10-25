<script lang="ts">
  import { dbToPercent, percentToDb } from './VolumeSlider.svelte';
  import LedMeter from './LedMeter.svelte';

  interface Props {
    /** Gain in dB (0 dB = 100 %). */
    db: number;
    levels?: number[];
    holds?: number[];
    color?: string;
    label: string;
    muted?: boolean;
    disabled?: boolean;
    /** Diameter in CSS pixels. */
    size?: number;
    onchange: (db: number) => void;
  }
  let { db, levels = [-120, -120], holds = [], color = 'var(--accent)', label, muted = false, disabled = false, size = 150, onchange }: Props = $props();

  // Like the Audiora logo: a knob inside a ring of rays. The rays light up to the volume.
  const RAYS = 29;
  const SWEEP = 270; // degrees from 0 % to 100 %
  const START = -135; // angle of 0 % (0 = straight up)
  const pct = $derived(dbToPercent(db));
  const angle = $derived(START + (pct / 100) * SWEEP);
  const lit = $derived(Math.round((pct / 100) * (RAYS - 1)));
  const rays = Array.from({ length: RAYS }, (_, i) => START + (i / (RAYS - 1)) * SWEEP);

  const C = 100; // svg centre; viewBox 200x200
  const polar = (deg: number, r: number) => {
    const a = ((deg - 90) * Math.PI) / 180;
    return [C + r * Math.cos(a), C + r * Math.sin(a)];
  };

  let dragging = $state(false);
  let startY = 0;
  let startPct = 0;

  function set(p: number) {
    onchange(percentToDb(Math.min(100, Math.max(0, Math.round(p)))));
  }

  function onpointerdown(e: PointerEvent) {
    if (disabled) return;
    (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
    dragging = true;
    startY = e.clientY;
    startPct = pct;
  }

  function onpointermove(e: PointerEvent) {
    if (!dragging) return;
    // 2 px per percent; hold Shift for fine control
    set(startPct + (startY - e.clientY) / (e.shiftKey ? 8 : 2));
  }

  function onkeydown(e: KeyboardEvent) {
    const step = e.shiftKey ? 10 : 2;
    const map: Record<string, number> = { ArrowUp: pct + step, ArrowRight: pct + step, ArrowDown: pct - step, ArrowLeft: pct - step, Home: 100, End: 0, PageUp: pct + 10, PageDown: pct - 10 };
    if (e.key in map) {
      e.preventDefault();
      set(map[e.key]);
    }
  }
</script>

<div class="vk" class:muted class:disabled style:--c={color}>
  <div
    class="dial"
    class:dragging
    style:width="min(100%, {size}px)"
    role="slider"
    tabindex={disabled ? -1 : 0}
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={pct}
    aria-valuetext="{pct} %"
    aria-disabled={disabled}
    {onpointerdown}
    {onpointermove}
    onpointerup={() => (dragging = false)}
    onpointercancel={() => (dragging = false)}
    ondblclick={() => !disabled && onchange(0)}
    onwheel={(e) => {
      if (disabled) return;
      e.preventDefault();
      set(pct + (e.deltaY < 0 ? 2 : -2));
    }}
    {onkeydown}
  >
    <svg viewBox="0 0 200 200" aria-hidden="true">
      <defs>
        <radialGradient id="knob-face" cx="50%" cy="38%" r="65%">
          <stop offset="0%" stop-color="#2a313c" />
          <stop offset="70%" stop-color="#151a21" />
          <stop offset="100%" stop-color="#0c0f14" />
        </radialGradient>
      </defs>
      {#each rays as a, i (i)}
        {@const [x1, y1] = polar(a, 70)}
        {@const [x2, y2] = polar(a, i % 7 === 0 ? 96 : 90)}
        <line class="ray" class:on={i <= lit && pct > 0} {x1} {y1} {x2} {y2} />
      {/each}
      <circle class="rim" cx={C} cy={C} r="61" />
      <circle cx={C} cy={C} r="58" fill="url(#knob-face)" />
      <circle class="sheen" cx={C} cy={C} r="57" />
      {#each [angle] as a (0)}
        {@const [nx, ny] = polar(a, 46)}
        <line class="needle" x1={C} y1={C} x2={nx} y2={ny} />
      {/each}
      <circle class="hub" cx={C} cy={C} r="5" />
    </svg>
  </div>

  <output class="num">{pct}<small>%</small></output>
  <div class="meter" title="Level">
    <LedMeter orientation="horizontal" levels={muted ? [-120, -120] : levels} holds={muted ? [] : holds} thickness={4} />
  </div>
</div>

<style>
  .vk {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    width: 100%;
  }

  .dial {
    aspect-ratio: 1;
    cursor: ns-resize;
    touch-action: none;
    border-radius: 50%;
  }

  .dial:focus-visible {
    outline: none;
    box-shadow: 0 0 0 2px color-mix(in oklab, var(--c) 60%, transparent);
  }

  svg {
    display: block;
    width: 100%;
    height: 100%;
    overflow: visible;
  }

  .ray {
    stroke: rgba(255, 255, 255, 0.1);
    stroke-width: 5;
    stroke-linecap: round;
    transition: stroke 0.12s;
  }

  .ray.on {
    stroke: var(--c);
    filter: drop-shadow(0 0 3px color-mix(in oklab, var(--c) 70%, transparent));
  }

  .rim {
    fill: none;
    stroke: rgba(255, 255, 255, 0.85);
    stroke-width: 3;
  }

  .sheen {
    fill: none;
    stroke: rgba(255, 255, 255, 0.06);
    stroke-width: 1;
  }

  .needle {
    stroke: #fff;
    stroke-width: 6;
    stroke-linecap: round;
  }

  .hub {
    fill: #fff;
  }

  .dragging .rim {
    stroke: var(--c);
  }

  output {
    font-size: 18px;
    font-weight: 600;
    color: var(--text);
  }

  output small {
    font-size: 11px;
    color: var(--faint);
    margin-left: 1px;
  }

  .meter {
    width: 78%;
    max-width: 150px;
  }

  .muted .ray.on {
    stroke: var(--faint);
    filter: none;
  }

  .muted .needle,
  .muted .hub {
    stroke: var(--faint);
    fill: var(--faint);
  }

  .muted output {
    color: var(--faint);
    text-decoration: line-through;
  }

  .disabled {
    opacity: 0.35;
  }

  .disabled .dial {
    cursor: default;
  }
</style>
