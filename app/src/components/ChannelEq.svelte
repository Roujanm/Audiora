<script lang="ts">
  import { BASIC_EQ } from '../lib/channels.ts';
  import { EQ_BANDS, eqResponseDb, freqToX, xToFreq, type Band } from '../lib/dsp.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import Card from './Card.svelte';
  import EqEditor from './EqEditor.svelte';
  import Segmented from './Segmented.svelte';
  import Slider from './Slider.svelte';
  import VSlider from './VSlider.svelte';

  interface Props {
    strip: number;
    color: string;
  }
  let { strip, color }: Props = $props();

  type Tab = 'basic' | 'bands' | 'param';
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

  // ---- live curve ---------------------------------------------------------
  let canvas = $state<HTMLCanvasElement>();
  let box = $state<HTMLDivElement>();
  let width = $state(600);
  const height = 150;

  $effect(() => {
    if (tab === 'param') return;
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
    if (!canvas || tab === 'param') return;
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
    for (const f of [50, 100, 200, 500, 1000, 2000, 5000, 10000]) {
      const gx = Math.round(x(f)) + 0.5;
      ctx.strokeStyle = 'rgba(255,255,255,0.05)';
      ctx.beginPath();
      ctx.moveTo(gx, 0);
      ctx.lineTo(gx, height);
      ctx.stroke();
      ctx.fillStyle = 'rgba(133,145,163,0.75)';
      ctx.fillText(f >= 1000 ? `${f / 1000}k` : `${f}`, gx + 3, height - 5);
    }
    ctx.strokeStyle = 'rgba(255,255,255,0.18)';
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
      const sa = new Path2D(sp);
      sa.lineTo(x(hz[hz.length - 1]), height);
      sa.lineTo(x(hz[0]), height);
      sa.closePath();
      const g = ctx.createLinearGradient(0, height * 0.15, 0, height);
      g.addColorStop(0, 'rgba(160,180,210,0.28)');
      g.addColorStop(1, 'rgba(160,180,210,0.02)');
      ctx.fillStyle = g;
      ctx.fill(sa);
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
    ctx.fillStyle = color + '2a';
    ctx.fill(area);
    ctx.lineJoin = 'round';
    ctx.shadowColor = color;
    ctx.shadowBlur = 12;
    ctx.strokeStyle = color;
    ctx.lineWidth = 2.4;
    ctx.stroke(path);
    ctx.shadowBlur = 0;

    bands.forEach((b, i) => {
      const dim = Math.abs(b.gainDb) < 0.01;
      ctx.beginPath();
      ctx.arc(x(b.freq), y(Math.max(-RANGE_DB, Math.min(RANGE_DB, b.gainDb))), 4.5, 0, Math.PI * 2);
      ctx.fillStyle = dim ? 'rgba(180,190,205,0.55)' : '#fff';
      ctx.fill();
      void i;
    });
  });
</script>

<Card title="Equalizer" icon="eq" {color} description="Shape the sound of this channel. Grey: live spectrum of the channel.">
  {#snippet actions()}
    <Segmented
      label="Equalizer view"
      size="sm"
      value={tab}
      options={[
        { id: 'basic', label: 'Basic' },
        { id: 'bands', label: 'Bands' },
        { id: 'param', label: 'Parametric' },
      ]}
      onchange={(id) => (tab = id as Tab)}
    />
    <button class="btn sm" disabled={flat} onclick={() => Array.from({ length: EQ_BANDS }, (_, b) => b).forEach((b) => setGain(b)(0))}>Flatten</button>
  {/snippet}

  {#if tab === 'param'}
    <EqEditor {strip} />
  {:else}
    <div class="curve inset" bind:this={box}>
      <canvas bind:this={canvas} style:width="{width}px" style:height="{height}px" aria-label="Equalizer response of this channel"></canvas>
    </div>

    {#if tab === 'basic'}
      <div class="basic">
        {#each BASIC_EQ as g (g.id)}
          <Slider label={g.label} value={basicValue(g)} min={-12} max={12} step={0.5} unit=" dB" reset={0} onchange={(v) => setBasic(g, v)} />
        {/each}
      </div>
    {:else}
      <div class="bands">
        {#each bands as b, i (i)}
          <VSlider label={NAMES[i]} sub={fmtFreq(b.freq)} value={b.gainDb} {color} onchange={setGain(i)} />
        {/each}
      </div>
    {/if}
  {/if}
</Card>

<style>
  .curve {
    height: 150px;
    overflow: hidden;
    margin-bottom: 14px;
  }

  canvas {
    display: block;
  }

  .basic {
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 640px;
  }

  .basic :global(.slider) {
    grid-template-columns: 70px 1fr 70px;
  }

  .bands {
    display: grid;
    grid-template-columns: repeat(6, minmax(0, 1fr));
    gap: 4px;
  }
</style>
