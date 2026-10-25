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
  interface Props {
    /** Gain in dB (0 dB = 100 %). */
    db: number;
    /** Live level in dBFS for the meter inside the track. */
    level?: number;
    color?: string;
    label: string;
    muted?: boolean;
    disabled?: boolean;
    onchange: (db: number) => void;
  }
  let { db, level = -120, color = 'var(--accent)', label, muted = false, disabled = false, onchange }: Props = $props();

  const pct = $derived(dbToPercent(db));
  // meter: -60..0 dBFS over the full height
  const meter = $derived(Math.min(1, Math.max(0, (level + 60) / 60)));

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
      <i class="meter" style:height="{meter * 100}%"></i>
      <i class="thumb" style:bottom="{pct}%"></i>
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

  .hit {
    flex: 1;
    min-height: 100px;
    width: 64px;
    display: flex;
    justify-content: center;
    padding: 12px 0;
    cursor: ns-resize;
    touch-action: none;
  }

  .track {
    position: relative;
    width: 14px;
    height: 100%;
    border-radius: 999px;
    background: var(--bg-0);
    box-shadow: inset 0 2px 5px rgba(0, 0, 0, 0.85), 0 1px 0 rgba(255, 255, 255, 0.04);
  }

  .fill {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    border-radius: 999px;
    background: linear-gradient(to top, color-mix(in oklab, var(--c) 30%, transparent), color-mix(in oklab, var(--c) 70%, transparent));
    transition: height 0.05s linear;
  }

  /* live level, drawn as a bright core inside the volume fill */
  .meter {
    position: absolute;
    left: 4px;
    right: 4px;
    bottom: 3px;
    max-height: calc(100% - 6px);
    border-radius: 999px;
    background: linear-gradient(to top, #fff, color-mix(in oklab, var(--c) 40%, #fff));
    opacity: 0.85;
    box-shadow: 0 0 10px var(--c);
    transition: height 0.06s linear;
  }

  .thumb {
    position: absolute;
    left: 50%;
    width: 30px;
    height: 30px;
    border-radius: 50%;
    transform: translate(-50%, 50%);
    background: radial-gradient(circle at 50% 35%, #ffffff, #c9d1dd 70%);
    box-shadow: 0 4px 12px rgba(0, 0, 0, 0.6), 0 0 0 4px color-mix(in oklab, var(--c) 35%, transparent);
    pointer-events: none;
    transition: box-shadow 0.15s;
  }

  .hit:focus-visible {
    outline: none;
  }

  .hit:focus-visible .thumb,
  .hit.dragging .thumb {
    box-shadow: 0 4px 14px rgba(0, 0, 0, 0.7), 0 0 0 6px color-mix(in oklab, var(--c) 55%, transparent);
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

  .muted .fill,
  .muted .meter {
    filter: grayscale(1);
    opacity: 0.35;
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
