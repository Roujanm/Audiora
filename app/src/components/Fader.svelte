<script lang="ts">
  import { formatDb } from '../lib/dsp.ts';

  interface Props {
    /** Gain in dB; values at or below -100 mean "off" (minus infinity). */
    value: number;
    min?: number;
    max?: number;
    label?: string;
    /** Colour of the cap's indicator line and the level fill. */
    color?: string;
    onchange: (db: number) => void;
  }
  let { value, min = -60, max = 12, label = 'Level', color = 'var(--accent)', onchange }: Props = $props();

  const marks = [12, 6, 0, -6, -12, -24, -36, -48];
  const frac = $derived(value <= -100 ? 0 : (Math.min(Math.max(value, min), max) - min) / (max - min));
  const zero = $derived((0 - min) / (max - min));

  let el: HTMLDivElement;
  let dragging = $state(false);

  function emit(db: number) {
    const clamped = Math.min(Math.max(db, min), max);
    onchange(clamped <= min + 0.4 ? -120 : Math.round(clamped * 10) / 10);
  }

  function fromPointer(e: PointerEvent) {
    const r = el.getBoundingClientRect();
    // the cap is 30 px tall; keep its centre under the pointer
    const t = 1 - (e.clientY - r.top - 15) / (r.height - 30);
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

<div class="wrap" style:--c={color}>
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
    aria-valuetext="{formatDb(value)} dB"
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
    <div class="travel">
      <div class="groove"></div>
      <div class="fill" style:height="{frac * 100}%"></div>
      <div class="unity" style:bottom="{zero * 100}%"></div>
      {#each marks as m (m)}
        <span class="tick" class:zero={m === 0} style:bottom="{((m - min) / (max - min)) * 100}%">{m === 0 ? '0' : Math.abs(m)}</span>
      {/each}
      <div class="cap" style:bottom="calc({frac * 100}% - 15px)">
        <i class="grip"></i><i class="line"></i><i class="grip"></i>
      </div>
    </div>
  </div>
  <output class="num readout" class:off={value <= -100}>{formatDb(value)}<small>dB</small></output>
</div>

<style>
  .wrap {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
    height: 100%;
    min-height: 0;
  }

  .fader {
    position: relative;
    flex: 1;
    min-height: 150px;
    width: 64px;
    padding: 15px 0;
    touch-action: none;
    cursor: ns-resize;
  }

  .travel {
    position: relative;
    height: 100%;
  }

  .groove {
    position: absolute;
    left: 14px;
    top: 0;
    bottom: 0;
    width: 6px;
    border-radius: 3px;
    background: var(--bg-0);
    box-shadow: inset 0 2px 4px rgba(0, 0, 0, 0.9), 0 1px 0 rgba(255, 255, 255, 0.05);
  }

  .fill {
    position: absolute;
    left: 15.5px;
    bottom: 0;
    width: 3px;
    border-radius: 2px;
    background: linear-gradient(to top, color-mix(in oklab, var(--c) 40%, transparent), var(--c));
    box-shadow: 0 0 10px color-mix(in oklab, var(--c) 55%, transparent);
    transition: height 0.04s linear;
  }

  .unity {
    position: absolute;
    left: 6px;
    width: 22px;
    height: 1px;
    background: var(--line-3);
    transform: translateY(0.5px);
    pointer-events: none;
  }

  .tick {
    position: absolute;
    left: 36px;
    font: 500 9px/1 var(--mono);
    color: var(--faint);
    transform: translateY(50%);
    pointer-events: none;
  }

  .tick.zero {
    color: var(--text-2);
  }

  .cap {
    position: absolute;
    left: 0;
    width: 34px;
    height: 30px;
    border-radius: 6px;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.55), rgba(255, 255, 255, 0) 38%),
      linear-gradient(180deg, #dfe5ee, #8e99a9 52%, #c7cfdb);
    box-shadow:
      0 6px 10px -2px rgba(0, 0, 0, 0.75),
      0 1px 0 rgba(255, 255, 255, 0.6) inset,
      0 -1px 0 rgba(0, 0, 0, 0.35) inset;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    padding: 5px 5px;
    pointer-events: none;
    transition: box-shadow 0.15s;
  }

  .grip {
    display: block;
    height: 2px;
    border-radius: 1px;
    background: rgba(0, 0, 0, 0.28);
    box-shadow: 0 1px 0 rgba(255, 255, 255, 0.5);
  }

  .line {
    display: block;
    height: 3px;
    border-radius: 2px;
    background: var(--c);
    box-shadow: 0 0 8px var(--c);
  }

  .fader:focus-visible {
    outline: none;
  }

  .fader:focus-visible .cap,
  .fader.dragging .cap {
    box-shadow:
      0 0 0 2px var(--accent),
      0 6px 14px -2px rgba(0, 0, 0, 0.8),
      0 1px 0 rgba(255, 255, 255, 0.6) inset;
  }

  .readout {
    min-width: 62px;
    padding: 4px 6px 3px;
    text-align: center;
    font-size: 12.5px;
    font-weight: 600;
    border-radius: var(--r-sm);
    background: var(--bg-0);
    border: 1px solid var(--line);
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.6);
  }

  .readout small {
    margin-left: 2px;
    font-size: 9px;
    font-weight: 500;
    color: var(--faint);
  }

  .readout.off {
    color: var(--faint);
  }

  @media (max-height: 780px) {
    .fader {
      min-height: 90px;
    }

    .wrap {
      gap: 5px;
    }
  }
</style>
