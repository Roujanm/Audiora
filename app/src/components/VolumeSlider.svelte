<script lang="ts" module>
  /** Volume as people think of it: 0..100 %, with a taper that feels even (amplitude = p³). */
  export function dbToPercent(db: number): number {
    if (db <= -100) return 0;
    return Math.round(Math.min(1, Math.pow(10, db / 60)) * 100);
  }

  export function percentToDb(pct: number): number {
    if (pct <= 0) return -120;
    return Math.round(60 * Math.log10(Math.min(pct, 100) / 100) * 10) / 10;
  }
</script>

<script lang="ts">
  import LedMeter from './LedMeter.svelte';

  interface Props {
    /** Gain in dB (0 dB = 100 %). */
    db: number;
    /** Live levels per side in dBFS, shown on a separate meter beside the fader. */
    levels?: number[];
    holds?: number[];
    color?: string;
    label: string;
    muted?: boolean;
    disabled?: boolean;
    onchange: (db: number) => void;
  }
  let { db, levels = [-120, -120], holds = [], color = 'var(--accent)', label, muted = false, disabled = false, onchange }: Props = $props();

  const pct = $derived(dbToPercent(db));
  const MARKS = [100, 75, 50, 25, 0];

  let track: HTMLDivElement;
  let dragging = $state(false);

  function fromPointer(e: PointerEvent) {
    const r = track.getBoundingClientRect();
    const p = Math.round((1 - (e.clientY - r.top) / r.height) * 100);
    onchange(percentToDb(Math.min(100, Math.max(0, p))));
  }

  function onkeydown(e: KeyboardEvent) {
    const step = e.shiftKey ? 10 : 2;
    const map: Record<string, number> = { ArrowUp: pct + step, ArrowRight: pct + step, ArrowDown: pct - step, ArrowLeft: pct - step, Home: 100, End: 0, PageUp: pct + 10, PageDown: pct - 10 };
    if (e.key in map) {
      e.preventDefault();
      onchange(percentToDb(Math.min(100, Math.max(0, map[e.key]))));
    }
  }
</script>

<div class="vol" class:muted class:disabled style:--c={color}>
  <div class="body">
    <div class="marks" aria-hidden="true">
      {#each MARKS as m (m)}<span style:bottom="{m}%">{m}</span>{/each}
    </div>
    <div
      class="hit"
      class:dragging
      role="slider"
      tabindex={disabled ? -1 : 0}
      aria-label={label}
      aria-orientation="vertical"
      aria-valuemin={0}
      aria-valuemax={100}
      aria-valuenow={pct}
      aria-valuetext="{pct} %"
      aria-disabled={disabled}
      onpointerdown={(e) => {
        if (disabled) return;
        (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
        dragging = true;
        fromPointer(e);
      }}
      onpointermove={(e) => dragging && fromPointer(e)}
      onpointerup={() => (dragging = false)}
      onpointercancel={() => (dragging = false)}
      ondblclick={() => !disabled && onchange(0)}
      onwheel={(e) => {
        if (disabled) return;
        e.preventDefault();
        onchange(percentToDb(Math.min(100, Math.max(0, pct + (e.deltaY < 0 ? 2 : -2)))));
      }}
      {onkeydown}
    >
      <div class="track" bind:this={track}>
        <i class="fill" style:height="{pct}%"></i>
        <i class="cap" style:bottom="{pct}%"><b></b></i>
      </div>
    </div>
    <div class="meter">
      <LedMeter levels={muted ? [-120, -120] : levels} holds={muted ? [] : holds} thickness={4} scale={false} />
    </div>
  </div>
  <output class="num">{pct}<small>%</small></output>
</div>

<style>
  .vol {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
  }

  .body {
    flex: 1;
    min-height: 100px;
    display: flex;
    gap: 6px;
    padding: 14px 0;
  }

  .marks {
    position: relative;
    width: 20px;
    font: 500 9px var(--mono);
    color: var(--faint);
  }

  .marks span {
    position: absolute;
    right: 0;
    transform: translateY(50%);
  }

  .hit {
    width: 40px;
    display: flex;
    justify-content: center;
    cursor: ns-resize;
    touch-action: none;
  }

  .track {
    position: relative;
    width: 4px;
    height: 100%;
    border-radius: 2px;
    background: var(--bg-0);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.9), 0 1px 0 rgba(255, 255, 255, 0.04);
  }

  .fill {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: 2px;
    background: var(--c);
    opacity: 0.85;
  }

  /* a console-style fader cap: flat, machined, with a coloured index line */
  .cap {
    position: absolute;
    left: 50%;
    width: 34px;
    height: 20px;
    transform: translate(-50%, 50%);
    border-radius: 5px;
    background: linear-gradient(180deg, #eef1f6 0%, #c3cad5 48%, #9aa3b1 52%, #c9d0da 100%);
    box-shadow: 0 3px 8px rgba(0, 0, 0, 0.65), 0 1px 0 rgba(255, 255, 255, 0.7) inset;
    pointer-events: none;
    transition: box-shadow 0.15s;
  }

  .cap b {
    position: absolute;
    left: 5px;
    right: 5px;
    top: 50%;
    height: 2px;
    margin-top: -1px;
    border-radius: 1px;
    background: var(--c);
  }

  .hit:focus-visible {
    outline: none;
  }

  .hit:focus-visible .cap,
  .hit.dragging .cap {
    box-shadow: 0 0 0 2px var(--c), 0 3px 10px rgba(0, 0, 0, 0.7);
  }

  .meter {
    display: flex;
    width: 11px;
  }

  output {
    font-size: 15px;
    font-weight: 600;
    color: var(--text);
  }

  output small {
    font-size: 10px;
    color: var(--faint);
    margin-left: 1px;
  }

  .muted .fill {
    background: var(--faint);
  }

  .muted output {
    color: var(--faint);
    text-decoration: line-through;
  }

  .disabled {
    opacity: 0.35;
  }

  .disabled .hit {
    cursor: default;
  }
</style>
