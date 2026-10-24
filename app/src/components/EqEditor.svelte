<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { eqResponseDb, freqToX, xToFreq, formatDb, type Band } from '../lib/dsp.ts';

  interface Props {
    strip: number;
  }
  let { strip }: Props = $props();

  const RANGE_DB = 18;
  const NAMES = ['Low shelf', 'Mid 1', 'Mid 2', 'High shelf'];
  const COLORS = ['#ff8f6b', '#ffd166', '#6ee7b7', '#7aa7ff'];

  const fs = $derived(engine.info?.buses[0]?.sample_rate ?? 48000);
  const bands = $derived<Band[]>(
    [0, 1, 2, 3].map((b) => ({
      freq: engine.get(`strip/${strip}/eq/${b}/freq`, 1000),
      gainDb: engine.get(`strip/${strip}/eq/${b}/gain_db`),
      q: engine.get(`strip/${strip}/eq/${b}/q`, 1),
    })),
  );

  let canvas: HTMLCanvasElement;
  let box: HTMLDivElement;
  let width = $state(600);
  let height = $state(240);
  let active = $state<number | null>(null);
  let hover = $state<number | null>(null);

  const x = (f: number) => freqToX(f) * width;
  const y = (db: number) => height / 2 - (db / RANGE_DB) * (height / 2);

  $effect(() => {
    const ro = new ResizeObserver(([e]) => {
      width = Math.max(300, Math.floor(e.contentRect.width));
      height = Math.max(180, Math.floor(e.contentRect.height));
    });
    ro.observe(box);
    return () => ro.disconnect();
  });

  $effect(() => {
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    const ctx = canvas.getContext('2d')!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);

    const css = getComputedStyle(canvas);
    const line = css.getPropertyValue('--line').trim() || '#2b3542';
    const faint = css.getPropertyValue('--faint').trim() || '#5b6778';
    const accent = css.getPropertyValue('--accent').trim() || '#5aa9ff';

    // grid
    ctx.lineWidth = 1;
    ctx.font = '10px ' + css.fontFamily;
    ctx.fillStyle = faint;
    for (const f of [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000]) {
      const gx = Math.round(x(f)) + 0.5;
      ctx.strokeStyle = line;
      ctx.beginPath();
      ctx.moveTo(gx, 0);
      ctx.lineTo(gx, height);
      ctx.stroke();
      ctx.fillText(f >= 1000 ? `${f / 1000}k` : `${f}`, gx + 3, height - 4);
    }
    for (let db = -RANGE_DB; db <= RANGE_DB; db += 6) {
      const gy = Math.round(y(db)) + 0.5;
      ctx.strokeStyle = db === 0 ? '#3d4a5a' : line;
      ctx.beginPath();
      ctx.moveTo(0, gy);
      ctx.lineTo(width, gy);
      ctx.stroke();
      ctx.fillStyle = faint;
      ctx.fillText(`${db > 0 ? '+' : ''}${db}`, 4, gy - 3);
    }

    // response curve
    const pts: [number, number][] = [];
    for (let i = 0; i <= width; i += 2) {
      const f = xToFreq(i / width);
      pts.push([i, y(Math.max(-RANGE_DB, Math.min(RANGE_DB, eqResponseDb(bands, fs, f))))]);
    }
    ctx.beginPath();
    pts.forEach(([px, py], i) => (i ? ctx.lineTo(px, py) : ctx.moveTo(px, py)));
    ctx.strokeStyle = accent;
    ctx.lineWidth = 2;
    ctx.stroke();
    ctx.lineTo(width, y(0));
    ctx.lineTo(0, y(0));
    ctx.closePath();
    ctx.fillStyle = 'rgba(90,169,255,0.12)';
    ctx.fill();

    // band handles
    bands.forEach((b, i) => {
      const cx = x(b.freq);
      const cy = y(b.gainDb);
      ctx.beginPath();
      ctx.arc(cx, cy, active === i || hover === i ? 9 : 7, 0, Math.PI * 2);
      ctx.fillStyle = COLORS[i];
      ctx.globalAlpha = Math.abs(b.gainDb) < 0.01 ? 0.55 : 1;
      ctx.fill();
      ctx.globalAlpha = 1;
      ctx.lineWidth = 2;
      ctx.strokeStyle = '#0d1117';
      ctx.stroke();
      ctx.fillStyle = '#0d1117';
      ctx.font = 'bold 9px ' + css.fontFamily;
      ctx.textAlign = 'center';
      ctx.fillText(String(i + 1), cx, cy + 3);
      ctx.textAlign = 'left';
    });
  });

  function pick(e: MouseEvent): number | null {
    const r = canvas.getBoundingClientRect();
    const px = e.clientX - r.left;
    const py = e.clientY - r.top;
    let best: number | null = null;
    let bestD = 16;
    bands.forEach((b, i) => {
      const d = Math.hypot(x(b.freq) - px, y(b.gainDb) - py);
      if (d < bestD) {
        bestD = d;
        best = i;
      }
    });
    return best;
  }

  function drag(e: PointerEvent) {
    if (active === null) return;
    const r = canvas.getBoundingClientRect();
    const fx = Math.min(Math.max((e.clientX - r.left) / width, 0), 1);
    const gy = Math.min(Math.max((e.clientY - r.top) / height, 0), 1);
    const freq = Math.round(xToFreq(fx));
    const gain = Math.round(((0.5 - gy) * 2 * RANGE_DB) * 10) / 10;
    engine.set(`strip/${strip}/eq/${active}/freq`, freq);
    engine.set(`strip/${strip}/eq/${active}/gain_db`, gain);
  }

  function onwheel(e: WheelEvent) {
    const i = hover ?? active;
    if (i === null) return;
    e.preventDefault();
    const q = bands[i].q * (e.deltaY < 0 ? 1.1 : 1 / 1.1);
    engine.set(`strip/${strip}/eq/${i}/q`, Math.round(Math.min(Math.max(q, 0.1), 18) * 100) / 100);
  }

  function resetBand(i: number) {
    engine.set(`strip/${strip}/eq/${i}/gain_db`, 0);
  }

  const dirty = $derived(bands.some((b) => Math.abs(b.gainDb) >= 0.01));
</script>

<div class="eq">
  <div class="top">
    <h2>Equalizer</h2>
    <button class="btn" disabled={!dirty} onclick={() => [0, 1, 2, 3].forEach(resetBand)}>Flatten</button>
  </div>
  <div class="canvas" bind:this={box}>
    <canvas
      bind:this={canvas}
      style:width="{width}px"
      style:height="{height}px"
      aria-label="Equalizer curve. Drag the numbered points; scroll over a point to change Q."
      onpointerdown={(e) => {
        const i = pick(e);
        if (i !== null) {
          active = i;
          canvas.setPointerCapture(e.pointerId);
        }
      }}
      onpointermove={(e) => (active !== null ? drag(e) : (hover = pick(e)))}
      onpointerup={() => (active = null)}
      onpointercancel={() => (active = null)}
      ondblclick={(e) => {
        const i = pick(e);
        if (i !== null) resetBand(i);
      }}
      {onwheel}
    ></canvas>
  </div>
  <div class="bands">
    {#each bands as b, i (i)}
      <div class="band" style:--c={COLORS[i]} class:flat={Math.abs(b.gainDb) < 0.01}>
        <span class="dot"></span>
        <span class="n">{NAMES[i]}</span>
        <span class="num">{b.freq >= 1000 ? (b.freq / 1000).toFixed(2) + ' kHz' : Math.round(b.freq) + ' Hz'}</span>
        <span class="num">{formatDb(b.gainDb)} dB</span>
        <span class="num muted">Q {b.q.toFixed(2)}</span>
      </div>
    {/each}
  </div>
</div>

<style>
  .eq {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-height: 0;
  }

  .top {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .top h2 {
    margin: 0;
  }

  .canvas {
    position: relative;
    height: 240px;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    overflow: hidden;
  }

  canvas {
    display: block;
    touch-action: none;
    cursor: crosshair;
  }

  .bands {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
  }

  .band {
    display: grid;
    grid-template-columns: 10px 1fr;
    gap: 0 6px;
    font-size: 11px;
    padding: 6px 8px;
    border-radius: var(--radius-sm);
    background: var(--panel-2);
  }

  .band.flat {
    opacity: 0.6;
  }

  .dot {
    grid-row: span 4;
    width: 8px;
    height: 8px;
    margin-top: 4px;
    border-radius: 50%;
    background: var(--c);
  }

  .n {
    font-weight: 600;
  }
</style>
