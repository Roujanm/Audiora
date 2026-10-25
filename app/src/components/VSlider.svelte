<script lang="ts">
  import { formatDb } from '../lib/dsp.ts';

  interface Props {
    value: number;
    min?: number;
    max?: number;
    step?: number;
    label: string;
    /** Second line under the label, e.g. the band's centre frequency. */
    sub?: string;
    color?: string;
    height?: number;
    onchange: (v: number) => void;
  }
  let { value, min = -12, max = 12, step = 0.5, label, sub = '', color = 'var(--accent)', height = 150, onchange }: Props = $props();

  let track: HTMLDivElement;
  let dragging = $state(false);
  const frac = $derived((Math.min(Math.max(value, min), max) - min) / (max - min));
  const mid = $derived((0 - min) / (max - min));

  function emit(v: number) {
    const snapped = Math.round(Math.min(Math.max(v, min), max) / step) * step;
    onchange(Math.abs(snapped) < step / 2 ? 0 : snapped);
  }

  function fromPointer(e: PointerEvent) {
    const r = track.getBoundingClientRect();
    emit(min + (1 - (e.clientY - r.top) / r.height) * (max - min));
  }

  function onkeydown(e: KeyboardEvent) {
    const big = e.shiftKey ? step * 4 : step;
    const map: Record<string, number> = { ArrowUp: value + big, ArrowRight: value + big, ArrowDown: value - big, ArrowLeft: value - big, Home: max, End: min };
    if (e.key in map) {
      e.preventDefault();
      emit(map[e.key]);
    }
  }
</script>

<div class="vs" style:--c={color}>
  <output class="num val" class:zero={Math.abs(value) < 0.05}>{formatDb(value)}</output>
  <div
    class="hit"
    class:dragging
    style:height="{height}px"
    role="slider"
    tabindex="0"
    aria-label={label}
    aria-orientation="vertical"
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={value}
    aria-valuetext="{formatDb(value)} dB"
    onpointerdown={(e) => {
      (e.currentTarget as HTMLElement).setPointerCapture(e.pointerId);
      dragging = true;
      fromPointer(e);
    }}
    onpointermove={(e) => dragging && fromPointer(e)}
    onpointerup={() => (dragging = false)}
    onpointercancel={() => (dragging = false)}
    ondblclick={() => onchange(0)}
    onwheel={(e) => {
      e.preventDefault();
      emit(value + (e.deltaY < 0 ? step : -step));
    }}
    {onkeydown}
  >
    <div class="track" bind:this={track}>
      <i class="zero-line" style:bottom="{mid * 100}%"></i>
      <i
        class="fill"
        style:bottom="{Math.min(frac, mid) * 100}%"
        style:height="{Math.abs(frac - mid) * 100}%"
      ></i>
      <i class="thumb" style:bottom="calc({frac * 100}% - 8px)"></i>
    </div>
  </div>
  <span class="label">{label}</span>
  {#if sub}<span class="sub num">{sub}</span>{/if}
</div>

<style>
  .vs {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    min-width: 0;
  }

  .val {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--c);
  }

  .val.zero {
    color: var(--faint);
  }

  .hit {
    width: 44px;
    padding: 8px 0;
    box-sizing: content-box;
    cursor: ns-resize;
    touch-action: none;
    display: flex;
    justify-content: center;
  }

  .track {
    position: relative;
    width: 6px;
    height: 100%;
    border-radius: 3px;
    background: var(--bg-0);
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.9), 0 1px 0 rgba(255, 255, 255, 0.05);
  }

  .zero-line {
    position: absolute;
    left: -8px;
    right: -8px;
    height: 1px;
    background: var(--line-3);
  }

  .fill {
    position: absolute;
    left: 1.5px;
    width: 3px;
    border-radius: 2px;
    background: var(--c);
    box-shadow: 0 0 10px color-mix(in oklab, var(--c) 55%, transparent);
  }

  .thumb {
    position: absolute;
    left: -7px;
    width: 20px;
    height: 16px;
    border-radius: 5px;
    background: linear-gradient(180deg, #e6ebf3, #98a3b3);
    box-shadow: 0 3px 8px rgba(0, 0, 0, 0.7), 0 1px 0 rgba(255, 255, 255, 0.6) inset;
    pointer-events: none;
    transition: box-shadow 0.15s;
  }

  .thumb::after {
    content: '';
    position: absolute;
    inset: 6px 4px;
    border-radius: 1px;
    background: var(--c);
  }

  .hit:focus-visible {
    outline: none;
  }

  .hit:focus-visible .thumb,
  .hit.dragging .thumb {
    box-shadow: 0 0 0 2px var(--c), 0 3px 8px rgba(0, 0, 0, 0.7);
  }

  .label {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-2);
    text-align: center;
    line-height: 1.15;
  }

  .sub {
    font-size: 10px;
    color: var(--faint);
    margin-top: -3px;
  }
</style>
