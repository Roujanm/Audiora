<script lang="ts">
  import { formatDb } from '../lib/dsp.ts';

  interface Props {
    /** Gain in dB; values at or below -100 mean "off" (−∞). */
    value: number;
    min?: number;
    max?: number;
    label?: string;
    onchange: (db: number) => void;
  }
  let { value, min = -60, max = 12, label = 'Level', onchange }: Props = $props();

  const marks = [12, 6, 0, -6, -12, -24, -36, -48];
  const frac = $derived(value <= -100 ? 0 : (Math.min(Math.max(value, min), max) - min) / (max - min));

  let el: HTMLDivElement;
  let dragging = $state(false);

  function emit(db: number) {
    const clamped = Math.min(Math.max(db, min), max);
    onchange(clamped <= min + 0.4 ? -120 : Math.round(clamped * 10) / 10);
  }

  function fromPointer(e: PointerEvent) {
    const r = el.getBoundingClientRect();
    const t = 1 - (e.clientY - r.top) / r.height;
    emit(min + Math.min(Math.max(t, 0), 1) * (max - min));
  }

  function onpointerdown(e: PointerEvent) {
    el.setPointerCapture(e.pointerId);
    dragging = true;
    fromPointer(e);
  }

  function onkeydown(e: KeyboardEvent) {
    const step = e.shiftKey ? 0.1 : 1;
    const cur = value <= -100 ? min : value;
    const map: Record<string, number> = {
      ArrowUp: cur + step,
      ArrowRight: cur + step,
      ArrowDown: cur - step,
      ArrowLeft: cur - step,
      PageUp: cur + 6,
      PageDown: cur - 6,
      Home: max,
      End: min,
    };
    if (e.key in map) {
      e.preventDefault();
      emit(map[e.key]);
    }
  }
</script>

<div class="wrap">
  <div
    class="fader"
    class:dragging
    bind:this={el}
    role="slider"
    tabindex="0"
    aria-label={label}
    aria-orientation="vertical"
    aria-valuemin={min}
    aria-valuemax={max}
    aria-valuenow={value <= -100 ? min : value}
    aria-valuetext={formatDb(value) + ' dB'}
    {onpointerdown}
    onpointermove={(e) => dragging && fromPointer(e)}
    onpointerup={() => (dragging = false)}
    onpointercancel={() => (dragging = false)}
    ondblclick={() => emit(0)}
    onwheel={(e) => {
      e.preventDefault();
      emit((value <= -100 ? min : value) + (e.deltaY < 0 ? 0.5 : -0.5));
    }}
    {onkeydown}
  >
    <div class="rail"></div>
    <div class="fill" style:height="{frac * 100}%"></div>
    {#each marks as m (m)}
      <span class="tick" class:zero={m === 0} style:bottom="{((m - min) / (max - min)) * 100}%">
        <i></i>{m === 0 ? '0' : Math.abs(m)}
      </span>
    {/each}
    <div class="thumb" style:bottom="calc({frac * 100}% - 9px)"></div>
  </div>
  <output class="num readout">{formatDb(value)}</output>
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    height: 100%;
    min-height: 0;
  }

  .fader {
    position: relative;
    flex: 1;
    min-height: 120px;
    width: 52px;
    touch-action: none;
    cursor: ns-resize;
    user-select: none;
  }

  .rail {
    position: absolute;
    left: 8px;
    top: 0;
    bottom: 0;
    width: 6px;
    border-radius: 3px;
    background: var(--bg);
    border: 1px solid var(--line);
  }

  .fill {
    position: absolute;
    left: 9px;
    bottom: 0;
    width: 4px;
    border-radius: 2px;
    background: linear-gradient(to top, #2b5f98, var(--accent));
    opacity: 0.8;
  }

  .tick {
    position: absolute;
    left: 20px;
    display: flex;
    align-items: center;
    gap: 3px;
    font-size: 9px;
    color: var(--faint);
    transform: translateY(50%);
    pointer-events: none;
  }

  .tick i {
    width: 5px;
    height: 1px;
    background: var(--faint);
  }

  .tick.zero {
    color: var(--text);
  }

  .tick.zero i {
    width: 8px;
    background: var(--text);
  }

  .thumb {
    position: absolute;
    left: 1px;
    width: 20px;
    height: 18px;
    border-radius: 4px;
    background: linear-gradient(#e9eef6, #b8c3d1);
    box-shadow: 0 1px 3px rgba(0, 0, 0, 0.6);
    pointer-events: none;
  }

  .thumb::after {
    content: '';
    position: absolute;
    left: 3px;
    right: 3px;
    top: 8px;
    height: 2px;
    background: #4b5667;
    border-radius: 1px;
  }

  .fader:focus-visible .thumb,
  .fader.dragging .thumb {
    outline: 2px solid var(--accent);
  }

  .readout {
    font-size: 12px;
    min-width: 46px;
    text-align: center;
    padding: 2px 4px;
    border-radius: 4px;
    background: var(--bg);
    border: 1px solid var(--line);
  }
</style>
