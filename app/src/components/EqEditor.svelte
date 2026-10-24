<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { eqResponseDb, formatDb, freqToX, xToFreq, type Band } from '../lib/dsp.ts';
  import Knob from './Knob.svelte';

  interface Props {
    strip: number;
  }
  let { strip }: Props = $props();

  const RANGE_DB = 18;
  const NAMES = ['Low shelf', 'Low-mid', 'High-mid', 'High shelf'];
  const COLORS = ['#ff8a65', '#ffd54f', '#5eead4', '#7aa7ff'];

  const fs = $derived(engine.info?.buses[0]?.sample_rate ?? 48000);
  const bands = $derived<Band[]>(
    [0, 1, 2, 3].map((b) => ({
      freq: engine.get(`strip/${strip}/eq/${b}/freq`, 1000),
      gainDb: engine.get(`strip/${strip}/eq/${b}/gain_db`),
      q: engine.get(`strip/${strip}/eq/${b}/q`, 1),
    })),
  );
  const flat = $derived(bands.every((b) => Math.abs(b.gainDb) < 0.01));

  let canvas: HTMLCanvasElement;
  let box: HTMLDivElement;
  let width = $state(700);
  let height = $state(280);
  let active = $state<number | null>(null);
  let hover = $state<number | null>(null);
  let cursor = $state<{ x: number; y: number } | null>(null);

  const x = (f: number) => freqToX(f) * width;
  const y = (db: number) => height / 2 - (db / RANGE_DB) * (height / 2);

  $effect(() => {
    const ro = new ResizeObserver(([e]) => {
      width = Math.max(320, Math.floor(e.contentRect.width));
      height = Math.max(200, Math.floor(e.contentRect.height));
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

    const root = getComputedStyle(document.documentElement);
    const accent = root.getPropertyValue('--accent').trim() || '#4d9bff';
    const accentHi = root.getPropertyValue('--accent-hi').trim() || accent;
    const font = root.getPropertyValue('--mono').trim();

    // grid
    ctx.lineWidth = 1;
    ctx.font = `500 10px ${font}`;
    for (const f of [20, 50, 100, 200, 500, 1000, 2000, 5000, 10000, 20000]) {
      const gx = Math.round(x(f)) + 0.5;
      ctx.strokeStyle = 'rgba(255,255,255,0.045)';
      ctx.beginPath();
      ctx.moveTo(gx, 0);
      ctx.lineTo(gx, height);
      ctx.stroke();
      ctx.fillStyle = 'rgba(133,145,163,0.8)';
      ctx.fillText(f >= 1000 ? `${f / 1000}k` : `${f}`, Math.min(gx + 4, width - 24), height - 7);
    }
    for (let db = -RANGE_DB; db <= RANGE_DB; db += 6) {
      const gy = Math.round(y(db)) + 0.5;
      ctx.strokeStyle = db === 0 ? 'rgba(255,255,255,0.22)' : 'rgba(255,255,255,0.045)';
      ctx.beginPath();
      ctx.moveTo(0, gy);
      ctx.lineTo(width, gy);
      ctx.stroke();
      if (db !== 0 && db !== -RANGE_DB) {
        ctx.fillStyle = 'rgba(133,145,163,0.8)';
        ctx.fillText(`${db > 0 ? '+' : ''}${db}`, 8, gy - 4);
      }
    }

    // response
    const pts: [number, number][] = [];
    for (let i = 0; i <= width; i += 2) {
      const f = xToFreq(i / width);
      pts.push([i, y(Math.max(-RANGE_DB, Math.min(RANGE_DB, eqResponseDb(bands, fs, f))))]);
    }
    const path = new Path2D();
    pts.forEach(([px, py], i) => (i ? path.lineTo(px, py) : path.moveTo(px, py)));

    const area = new Path2D(path);
    area.lineTo(width, y(0));
    area.lineTo(0, y(0));
    area.closePath();
    const fill = ctx.createLinearGradient(0, 0, 0, height);
    fill.addColorStop(0, 'rgba(255,255,255,0)');
    fill.addColorStop(0.5, 'rgba(255,255,255,0)');
    ctx.save();
    ctx.clip(area);
    const up = ctx.createLinearGradient(0, y(RANGE_DB), 0, y(0));
    up.addColorStop(0, accent + '66');
    up.addColorStop(1, accent + '08');
    ctx.fillStyle = up;
    ctx.fillRect(0, 0, width, y(0));
    const down = ctx.createLinearGradient(0, y(0), 0, y(-RANGE_DB));
    down.addColorStop(0, accent + '08');
    down.addColorStop(1, accent + '55');
    ctx.fillStyle = down;
    ctx.fillRect(0, y(0), width, height);
    ctx.restore();

    ctx.lineJoin = 'round';
    ctx.shadowColor = accent;
    ctx.shadowBlur = 14;
    ctx.strokeStyle = accentHi;
    ctx.lineWidth = 2.5;
    ctx.stroke(path);
    ctx.shadowBlur = 0;

    // handles
    bands.forEach((b, i) => {
      const cx = x(b.freq);
      const cy = y(b.gainDb);
      const on = active === i || hover === i;
      const dim = Math.abs(b.gainDb) < 0.01;
      ctx.beginPath();
      ctx.arc(cx, cy, on ? 15 : 12, 0, Math.PI * 2);
      ctx.fillStyle = COLORS[i] + (on ? '33' : '1c');
      ctx.fill();
      ctx.beginPath();
      ctx.arc(cx, cy, 7.5, 0, Math.PI * 2);
      ctx.fillStyle = COLORS[i];
      ctx.globalAlpha = dim ? 0.55 : 1;
      ctx.shadowColor = COLORS[i];
      ctx.shadowBlur = on ? 14 : 6;
      ctx.fill();
      ctx.shadowBlur = 0;
      ctx.globalAlpha = 1;
      ctx.lineWidth = 2;
      ctx.strokeStyle = '#0b0e13';
      ctx.stroke();
      ctx.fillStyle = '#0b0e13';
      ctx.font = `700 9px ${font}`;
      ctx.textAlign = 'center';
      ctx.fillText(String(i + 1), cx, cy + 3.2);
      ctx.textAlign = 'left';
    });

    // cursor read-out
    if (cursor && active === null && hover === null) {
      const f = xToFreq(cursor.x / width);
      const db = eqResponseDb(bands, fs, f);
      const label = `${f >= 1000 ? (f / 1000).toFixed(2) + ' kHz' : Math.round(f) + ' Hz'}   ${formatDb(db)} dB`;
      ctx.font = `500 11px ${font}`;
      const tw = ctx.measureText(label).width + 16;
      const tx = Math.min(Math.max(cursor.x + 12, 6), width - tw - 6);
      ctx.fillStyle = 'rgba(11,14,19,0.9)';
      ctx.strokeStyle = 'rgba(255,255,255,0.14)';
      ctx.beginPath();
      ctx.roundRect(tx, 10, tw, 24, 6);
      ctx.fill();
      ctx.stroke();
      ctx.fillStyle = '#eef2f8';
      ctx.fillText(label, tx + 8, 26);
      ctx.strokeStyle = 'rgba(255,255,255,0.12)';
      ctx.setLineDash([3, 4]);
      ctx.beginPath();
      ctx.moveTo(Math.round(cursor.x) + 0.5, 0);
      ctx.lineTo(Math.round(cursor.x) + 0.5, height);
      ctx.stroke();
      ctx.setLineDash([]);
    }
  });

  function pick(e: MouseEvent): number | null {
    const r = canvas.getBoundingClientRect();
    const px = e.clientX - r.left;
    const py = e.clientY - r.top;
    let best: number | null = null;
    let bestD = 20;
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
    engine.set(`strip/${strip}/eq/${active}/freq`, Math.round(xToFreq(fx)));
    engine.set(`strip/${strip}/eq/${active}/gain_db`, Math.round((0.5 - gy) * 2 * RANGE_DB * 10) / 10);
  }

  function onwheel(e: WheelEvent) {
    const i = hover ?? active;
    if (i === null) return;
    e.preventDefault();
    const q = bands[i].q * (e.deltaY < 0 ? 1.1 : 1 / 1.1);
    engine.set(`strip/${strip}/eq/${i}/q`, Math.round(Math.min(Math.max(q, 0.1), 18) * 100) / 100);
  }

  const set = (b: number, leaf: string) => (v: number) => engine.set(`strip/${strip}/eq/${b}/${leaf}`, v);
  const fmtFreq = (f: number) => (f >= 1000 ? `${(f / 1000).toFixed(f >= 10000 ? 1 : 2)}k` : `${Math.round(f)}`);
</script>

<section class="eq panel">
  <header>
    <div>
      <h2 class="eyebrow">Equalizer</h2>
      <p class="hint muted">Drag a point to set frequency and gain · scroll over it to change Q · double-click to reset</p>
    </div>
    <button class="btn sm" disabled={flat} onclick={() => [0, 1, 2, 3].forEach((b) => engine.set(`strip/${strip}/eq/${b}/gain_db`, 0))}>
      Flatten
    </button>
  </header>

  <div class="canvas inset" bind:this={box}>
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
      onpointermove={(e) => {
        const r = canvas.getBoundingClientRect();
        cursor = { x: e.clientX - r.left, y: e.clientY - r.top };
        if (active !== null) drag(e);
        else hover = pick(e);
      }}
      onpointerleave={() => {
        cursor = null;
        hover = null;
      }}
      onpointerup={() => (active = null)}
      onpointercancel={() => (active = null)}
      ondblclick={(e) => {
        const i = pick(e);
        if (i !== null) engine.set(`strip/${strip}/eq/${i}/gain_db`, 0);
      }}
      {onwheel}
    ></canvas>
  </div>

  <div class="bands">
    {#each bands as b, i (i)}
      <div class="band" style:--c={COLORS[i]} class:flat={Math.abs(b.gainDb) < 0.01} class:focus={hover === i || active === i}>
        <div class="bh"><span class="dot">{i + 1}</span><strong>{NAMES[i]}</strong></div>
        <div class="knobs">
          <Knob label="Freq" size={46} min={20} max={20000} scale="log" step={1} reset={[100, 400, 2500, 8000][i]} color={COLORS[i]} value={b.freq} format={fmtFreq} onchange={set(i, 'freq')} />
          <Knob label="Gain" size={46} min={-24} max={24} step={0.5} bipolar reset={0} color={COLORS[i]} value={b.gainDb} format={(v) => `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toFixed(1)}`} onchange={set(i, 'gain_db')} />
          <Knob label="Q" size={46} min={0.1} max={18} scale="log" step={0.05} digits={2} reset={i === 0 || i === 3 ? 0.707 : 1} color={COLORS[i]} value={b.q} onchange={set(i, 'q')} />
        </div>
      </div>
    {/each}
  </div>
</section>

<style>
  .eq {
    padding: 18px 20px 20px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 16px;
  }

  .hint {
    font-size: 12px;
    margin-top: 3px;
  }

  .canvas {
    height: 300px;
    overflow: hidden;
  }

  canvas {
    display: block;
    touch-action: none;
    cursor: crosshair;
  }

  .bands {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 10px;
  }

  .band {
    padding: 12px 8px 10px;
    border-radius: var(--r-md);
    background: var(--bg-2);
    border: 1px solid var(--line);
    transition: border-color 0.15s, opacity 0.15s;
  }

  .band.flat {
    opacity: 0.72;
  }

  .band.focus {
    opacity: 1;
    border-color: color-mix(in oklab, var(--c) 50%, transparent);
  }

  .bh {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 4px 10px;
    font-size: 12px;
  }

  .dot {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    background: var(--c);
    color: #0b0e13;
    font: 700 10px var(--mono);
    box-shadow: 0 0 12px -2px var(--c);
  }

  .knobs {
    display: flex;
    justify-content: space-around;
    gap: 2px;
  }
</style>
