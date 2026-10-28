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
        <i class="ticks"></i>
        <i class="fill" style:height="max({pct}%, {pct > 0 ? 30 : 0}px)"><b class="grip"></b></i>
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
    width: 44px;
    display: flex;
    justify-content: center;
    cursor: ns-resize;
    touch-action: none;
  }

  /* a full-height capsule that fills with the channel colour up to the volume */
  .track {
    position: relative;
    width: var(--track-w, 30px);
    height: 100%;
    border-radius: calc(var(--track-w, 30px) / 2);
    overflow: hidden;
    background: linear-gradient(180deg, #0a0d12, #11151c);
    box-shadow:
      inset 0 2px 6px rgba(0, 0, 0, 0.85),
      inset 0 0 0 1px rgba(255, 255, 255, 0.05),
      0 1px 0 rgba(255, 255, 255, 0.04);
  }

  .ticks {
    position: absolute;
    inset: 10px 9px;
    background: repeating-linear-gradient(to top, rgba(255, 255, 255, 0.07) 0 1px, transparent 1px 10%);
    pointer-events: none;
  }

  .fill {
    position: absolute;
    left: 3px;
    right: 3px;
    bottom: 3px;
    max-height: calc(100% - 6px);
    border-radius: 12px;
    background:
      linear-gradient(90deg, rgba(255, 255, 255, 0.18), transparent 45%, rgba(0, 0, 0, 0.18)),
      linear-gradient(0deg, color-mix(in oklab, var(--c) 45%, #000), var(--c));
    box-shadow: inset 0 1px 0 rgba(255, 255, 255, 0.35);
    transition: height 0.05s linear;
  }

  /* grip lines at the top of the fill: where you grab it */
  .grip {
    position: absolute;
    top: 7px;
    left: 50%;
    width: 12px;
    height: 6px;
    transform: translateX(-50%);
    border-top: 2px solid rgba(255, 255, 255, 0.95);
    border-bottom: 2px solid rgba(255, 255, 255, 0.95);
    border-radius: 1px;
    opacity: 0.9;
  }

  .hit:hover .fill {
    filter: brightness(1.1);
  }

  .hit:focus-visible {
    outline: none;
  }

  .hit:focus-visible .track,
  .hit.dragging .track {
    box-shadow:
      inset 0 2px 6px rgba(0, 0, 0, 0.85),
      0 0 0 2px color-mix(in oklab, var(--c) 70%, transparent);
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
    background: linear-gradient(0deg, #2a313b, #4a525e);
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
