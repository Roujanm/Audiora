<script lang="ts">
  interface Props {
    /** One level per channel, in dBFS. */
    levels: number[];
    /** Peak-hold markers in dBFS (same order). */
    holds?: number[];
    floor?: number;
    orientation?: 'vertical' | 'horizontal';
    /** Draw the dB scale beside the bars. */
    scale?: boolean;
    /** Thickness of each channel bar in CSS pixels. */
    thickness?: number;
  }
  let {
    levels,
    holds = [],
    floor = -60,
    orientation = 'vertical',
    scale = true,
    thickness = 8,
  }: Props = $props();

  const TICKS = [0, -6, -12, -24, -48];
  const SEG = 3; // segment length
  const GAP = 1.5;

  let canvas: HTMLCanvasElement;
  let box: HTMLDivElement;
  let w = $state(40);
  let h = $state(160);

  const vertical = $derived(orientation === 'vertical');

  $effect(() => {
    const ro = new ResizeObserver(([e]) => {
      w = Math.max(8, Math.floor(e.contentRect.width));
      h = Math.max(8, Math.floor(e.contentRect.height));
    });
    ro.observe(box);
    return () => ro.disconnect();
  });

  function colourFor(db: number): string {
    if (db > -1) return '#ff5a6b';
    if (db > -6) return '#ffb347';
    if (db > -14) return '#d3e65a';
    return '#3ddc97';
  }

  $effect(() => {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = w * dpr;
    canvas.height = h * dpr;
    const ctx = canvas.getContext('2d')!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, w, h);

    const n = levels.length;
    const span = vertical ? h : w;
    const count = Math.floor(span / (SEG + GAP));
    const lit = (db: number) => Math.round(Math.min(Math.max((db - floor) / -floor, 0), 1) * count);
    const gapBetween = 3;

    for (let ch = 0; ch < n; ch++) {
      const litSegs = lit(levels[ch]);
      const holdSeg = holds[ch] !== undefined && holds[ch] > floor ? Math.min(lit(holds[ch]), count) - 1 : -1;
      for (let i = 0; i < count; i++) {
        const db = floor + ((i + 1) / count) * -floor;
        const on = i < litSegs;
        const isHold = i === holdSeg && !on;
        ctx.fillStyle = on || isHold ? colourFor(db) : 'rgba(255,255,255,0.055)';
        ctx.globalAlpha = on ? 1 : isHold ? 0.95 : 1;
        if (vertical) {
          const x = ch * (thickness + gapBetween);
          const y = h - (i + 1) * (SEG + GAP) + GAP;
          ctx.fillRect(x, y, thickness, SEG);
        } else {
          const y = ch * (thickness + gapBetween);
          const x = i * (SEG + GAP);
          ctx.fillRect(x, y, SEG, thickness);
        }
      }
      ctx.globalAlpha = 1;
    }
  });

  const pos = (db: number) => Math.min(Math.max((db - floor) / -floor, 0), 1);
  const barsSize = $derived(levels.length * thickness + (levels.length - 1) * 3);
</script>

<div class="meter" class:horizontal={!vertical} role="img" aria-label="Level meter">
  <div class="bars" bind:this={box} style={vertical ? `width:${barsSize}px` : `height:${barsSize}px`}>
    <canvas bind:this={canvas} style:width="{w}px" style:height="{h}px"></canvas>
  </div>
  {#if scale && vertical}
    <div class="scale">
      {#each TICKS as t (t)}
        <span style:bottom="{pos(t) * 100}%">{Math.abs(t)}</span>
      {/each}
    </div>
  {/if}
</div>

<style>
  .meter {
    display: flex;
    gap: 6px;
    height: 100%;
    min-height: 0;
  }

  .meter.horizontal {
    flex-direction: column;
    height: auto;
    width: 100%;
  }

  .bars {
    position: relative;
    flex: none;
  }

  .horizontal .bars {
    width: 100%;
  }

  canvas {
    display: block;
  }

  .scale {
    position: relative;
    width: 16px;
    font-size: 9px;
    color: var(--faint);
    font-family: var(--mono);
    pointer-events: none;
  }

  .scale span {
    position: absolute;
    left: 0;
    transform: translateY(50%);
  }
</style>
