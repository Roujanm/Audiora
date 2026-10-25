<script lang="ts">
  import { BASIC_EQ } from '../lib/channels.ts';
  import { EQ_BANDS, eqResponseDb, freqToX, xToFreq, type Band } from '../lib/dsp.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import Icon from './Icon.svelte';
  import Segmented from './Segmented.svelte';
  import VSlider from './VSlider.svelte';

  interface Props {
    strip: number;
    color: string;
  }
  let { strip, color }: Props = $props();

  type Tab = 'basic' | 'advanced';
  let tab = $state<Tab>('basic');

  const NAMES = ['Sub bass', 'Bass', 'Low mids', 'Mid range', 'Upper mids', 'Highs'];
  const RANGE_DB = 15;
  const fmtFreq = (f: number) => (f >= 1000 ? `${(f / 1000).toFixed(f >= 10000 ? 0 : 1)} kHz` : `${Math.round(f)} Hz`);

  const fs = $derived(engine.info?.buses[0]?.sample_rate ?? 48000);
  const bands = $derived<Band[]>(
    Array.from({ length: EQ_BANDS }, (_, b) => ({
      freq: engine.get(`strip/${strip}/eq/${b}/freq`, 1000),
      gainDb: engine.get(`strip/${strip}/eq/${b}/gain_db`),
      q: engine.get(`strip/${strip}/eq/${b}/q`, 1),
    })),
  );
  const flat = $derived(bands.every((b) => Math.abs(b.gainDb) < 0.01));
  const setGain = (b: number) => (v: number) => engine.set(`strip/${strip}/eq/${b}/gain_db`, v);

  function basicValue(group: (typeof BASIC_EQ)[number]): number {
    return Math.round((bands[group.bands[0]].gainDb / group.weights[0]) * 2) / 2;
  }

  function setBasic(group: (typeof BASIC_EQ)[number], v: number) {
    group.bands.forEach((b, i) => setGain(b)(Math.round(v * group.weights[i] * 10) / 10));
  }

  // ---- response curve with the channel's live spectrum behind it --------------------
  let canvas = $state<HTMLCanvasElement>();
  let box = $state<HTMLDivElement>();
  let width = $state(600);
  const height = 130;

  $effect(() => {
    engine.setSpectrumSource(`strip/${strip}`);
    return () => engine.setSpectrumSource(null);
  });

  $effect(() => {
    if (!box) return;
    const ro = new ResizeObserver(([e]) => (width = Math.max(300, Math.floor(e.contentRect.width))));
    ro.observe(box);
    return () => ro.disconnect();
  });

  $effect(() => {
    if (!canvas) return;
    const dpr = window.devicePixelRatio || 1;
    canvas.width = width * dpr;
    canvas.height = height * dpr;
    const ctx = canvas.getContext('2d')!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, width, height);
    const x = (f: number) => freqToX(f) * width;
    const y = (db: number) => height / 2 - (db / RANGE_DB) * (height / 2);
    const mono = getComputedStyle(document.documentElement).getPropertyValue('--mono').trim();

    ctx.font = `500 9.5px ${mono}`;
    for (const f of [100, 1000, 10000]) {
      const gx = Math.round(x(f)) + 0.5;
      ctx.strokeStyle = 'rgba(255,255,255,0.05)';
      ctx.beginPath();
      ctx.moveTo(gx, 0);
      ctx.lineTo(gx, height);
      ctx.stroke();
      ctx.fillStyle = 'rgba(133,145,163,0.6)';
      ctx.fillText(f >= 1000 ? `${f / 1000}k` : `${f}`, gx + 4, height - 6);
    }
    ctx.strokeStyle = 'rgba(255,255,255,0.12)';
    ctx.beginPath();
    ctx.moveTo(0, y(0) + 0.5);
    ctx.lineTo(width, y(0) + 0.5);
    ctx.stroke();

    const spec = engine.spectrum;
    const hz = engine.bandHz;
    if (spec && hz.length === spec.length) {
      const sy = (db: number) => height - Math.min(Math.max((db + 90) / 90, 0), 1) * height * 0.85;
      const sp = new Path2D();
      spec.forEach((db, i) => (i ? sp.lineTo(x(hz[i]), sy(db)) : sp.moveTo(x(hz[i]), sy(db))));
      sp.lineTo(x(hz[hz.length - 1]), height);
      sp.lineTo(x(hz[0]), height);
      sp.closePath();
      ctx.fillStyle = 'rgba(160,180,210,0.14)';
      ctx.fill(sp);
    }

    const path = new Path2D();
    for (let i = 0; i <= width; i += 2) {
      const db = Math.max(-RANGE_DB, Math.min(RANGE_DB, eqResponseDb(bands, fs, xToFreq(i / width))));
      if (i) path.lineTo(i, y(db));
      else path.moveTo(i, y(db));
    }
    const area = new Path2D(path);
    area.lineTo(width, y(0));
    area.lineTo(0, y(0));
    area.closePath();
    ctx.fillStyle = color + '26';
    ctx.fill(area);
    ctx.lineJoin = 'round';
    ctx.shadowColor = color;
    ctx.shadowBlur = 12;
    ctx.strokeStyle = color;
    ctx.lineWidth = 2.5;
    ctx.stroke(path);
    ctx.shadowBlur = 0;
  });
</script>

<section class="eq panel" style:--c={color}>
  <header>
    <span class="ico"><Icon name="eq" size={17} /></span>
    <h3>Equalizer</h3>
    <Segmented
      label="Equalizer mode"
      size="sm"
      value={tab}
      options={[
        { id: 'basic', label: 'Basic' },
        { id: 'advanced', label: 'Advanced' },
      ]}
      onchange={(id) => (tab = id as Tab)}
    />
    <span class="grow"></span>
    <button class="btn sm ghost" disabled={flat} onclick={() => Array.from({ length: EQ_BANDS }, (_, b) => b).forEach((b) => setGain(b)(0))}>
      <Icon name="refresh" size={13} /> Flat
    </button>
  </header>

  <div class="curve" bind:this={box}>
    <canvas bind:this={canvas} style:width="{width}px" style:height="{height}px" aria-label="Equalizer response"></canvas>
  </div>

  {#if tab === 'basic'}
    <div class="sliders three">
      {#each BASIC_EQ as g (g.id)}
        <VSlider label={g.label} value={basicValue(g)} {color} height={130} onchange={(v) => setBasic(g, v)} />
      {/each}
    </div>
  {:else}
    <div class="sliders six">
      {#each bands as b, i (i)}
        <VSlider label={NAMES[i]} sub={fmtFreq(b.freq)} value={b.gainDb} {color} height={130} onchange={setGain(i)} />
      {/each}
    </div>
  {/if}
</section>

<style>
  .eq {
    padding: 14px 18px 18px;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  header {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .ico {
    width: 36px;
    height: 36px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    color: var(--accent-hi);
    background: var(--accent-soft);
  }

  h3 {
    font-size: 13.5px;
    font-weight: 650;
  }

  .grow {
    flex: 1;
  }

  .curve {
    height: 130px;
    border-radius: 12px;
    background: var(--bg-0);
    overflow: hidden;
  }

  canvas {
    display: block;
  }

  .sliders {
    display: grid;
    gap: 4px;
    margin: 0 auto;
    width: 100%;
  }

  .three {
    grid-template-columns: repeat(3, minmax(0, 1fr));
    max-width: 420px;
  }

  .six {
    grid-template-columns: repeat(6, minmax(0, 1fr));
  }
</style>
