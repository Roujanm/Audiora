<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { dbToPercent, percentToDb } from './VolumeSlider.svelte';
  import { shortName } from '../lib/theme.ts';
  import Icon from './Icon.svelte';

  const p = (leaf: string) => `bus/0/${leaf}`;
  const muted = $derived(engine.get(p('mute')) > 0.5);
  const device = $derived(shortName(engine.channels?.master_out.device_name ?? engine.info?.buses[0]?.name ?? 'Output'));
  const peak = $derived(Math.max(engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))));
  const clipping = $derived(engine.holdDb(p('peak_l')) > -0.5 || engine.holdDb(p('peak_r')) > -0.5);
  const lufs = $derived(engine.loudness.s <= -100 ? null : engine.loudness.s);
  const volume = $derived(Math.round(dbToPercent(engine.get(p('gain_db')))));
  const playing = $derived(engine.appGroups.filter((a) => a.active).slice(0, 5));

  // the master's live spectrum behind the stage
  $effect(() => {
    engine.setSpectrumSource('bus/0');
    return () => engine.setSpectrumSource(null);
  });

  let canvas = $state<HTMLCanvasElement>();
  $effect(() => {
    const c = canvas;
    if (!c) return;
    const ctx = c.getContext('2d');
    if (!ctx) return;
    let raf = 0;
    const draw = () => {
      const dpr = window.devicePixelRatio || 1;
      const w = c.clientWidth;
      const h = c.clientHeight;
      if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
        c.width = Math.round(w * dpr);
        c.height = Math.round(h * dpr);
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      const spec = engine.spectrum;
      const hz = engine.bandHz;
      if (spec && hz.length === spec.length && spec.length > 1) {
        // log-frequency bars, 30 Hz .. 18 kHz, mirrored glow from the bottom
        const lo = Math.log10(30);
        const hi = Math.log10(18000);
        const bars = 72;
        const gap = 3;
        const bw = (w - gap * (bars - 1)) / bars;
        const accent = getComputedStyle(c).getPropertyValue('--accent').trim() || '#dfe4ec';
        const grad = ctx.createLinearGradient(0, h, 0, 0);
        grad.addColorStop(0, accent);
        grad.addColorStop(1, 'rgba(255,255,255,0)');
        ctx.fillStyle = grad;
        let k = 0;
        for (let b = 0; b < bars; b++) {
          const f = 10 ** (lo + ((hi - lo) * (b + 0.5)) / bars);
          while (k < hz.length - 1 && hz[k + 1] < f) k++;
          const db = spec[k];
          const v = Math.max(0, Math.min(1, (db + 84) / 78));
          const bh = Math.max(2, v * h * 0.92);
          const x = b * (bw + gap);
          ctx.globalAlpha = 0.18 + v * 0.55;
          ctx.beginPath();
          ctx.roundRect(x, h - bh, bw, bh, Math.min(3, bw / 2));
          ctx.fill();
        }
        ctx.globalAlpha = 1;
      }
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  });

  // master loudness as an arc: -40 .. -6 LUFS
  const ARC = 2 * Math.PI * 52 * 0.75;
  const arcFill = $derived(lufs === null ? 0 : Math.max(0, Math.min(1, (lufs + 40) / 34)));
</script>

<section class="stage panel" class:muted aria-label="Master output">
  <canvas bind:this={canvas} aria-hidden="true"></canvas>

  <div class="who">
    <span class="eyebrow">Now playing on</span>
    <h2 title={device}>
      <Icon name={device.toLowerCase().includes('head') ? 'headphones' : 'speaker'} size={20} />
      {device}
    </h2>
    <div class="apps">
      {#each playing as a (a.name)}
        <span class="app" title={a.name}>{a.name}</span>
      {:else}
        <span class="muted quiet">Nothing is playing right now</span>
      {/each}
    </div>
  </div>

  <div class="loud" title="Short-term loudness of everything you hear (3 s)">
    <svg viewBox="0 0 120 120" aria-hidden="true">
      <circle class="track" cx="60" cy="60" r="52" stroke-dasharray="{ARC} 999" />
      <circle class="value" cx="60" cy="60" r="52" stroke-dasharray="{ARC * arcFill} 999" />
    </svg>
    <div class="read">
      <span class="num big">{lufs === null ? '—' : lufs.toFixed(0).replace('-', '−')}</span>
      <span class="eyebrow">LUFS</span>
    </div>
    <span class="clip" class:on={clipping}>{clipping ? 'Too loud' : peak > -60 ? 'Healthy level' : 'Silent'}</span>
  </div>

  <div class="vol">
    <div class="vol-head">
      <span class="eyebrow">Volume</span>
      <span class="num">{muted ? 'Muted' : `${volume}%`}</span>
    </div>
    <input
      type="range"
      min="0"
      max="100"
      step="1"
      value={volume}
      style:--fill="{volume}%"
      aria-label="Master volume"
      oninput={(e) => engine.set(p('gain_db'), percentToDb(+e.currentTarget.value))}
    />
    <button class="btn sm mute" class:on={muted} aria-pressed={muted} onclick={() => engine.toggle(p('mute'))}>
      <Icon name={muted ? 'mute' : 'volume'} size={15} />{muted ? 'Unmute' : 'Mute all'}
    </button>
  </div>
</section>

<style>
  .stage {
    position: relative;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto minmax(220px, 300px);
    align-items: center;
    gap: 28px;
    min-height: 168px;
    padding: 22px 26px;
    overflow: hidden;
  }

  canvas {
    position: absolute;
    left: 0;
    right: 0;
    bottom: 0;
    width: 100%;
    height: 70%;
    opacity: 0.55;
    pointer-events: none;
    mask-image: linear-gradient(90deg, transparent, #000 18%, #000 82%, transparent);
  }

  .stage.muted canvas {
    opacity: 0.15;
  }

  .who,
  .loud,
  .vol {
    position: relative;
  }

  .who h2 {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-top: 6px;
    font-size: 26px;
    font-weight: 700;
    letter-spacing: -0.02em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text);
  }

  .apps {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 12px;
  }

  .app {
    padding: 3px 10px;
    border-radius: 999px;
    font-size: 12px;
    font-weight: 600;
    text-transform: capitalize;
    color: var(--text);
    background: rgba(255, 255, 255, 0.07);
    border: 1px solid rgba(255, 255, 255, 0.1);
  }

  .quiet {
    font-size: 12px;
  }

  .loud {
    width: 132px;
    height: 132px;
    display: grid;
    place-items: center;
  }

  .loud svg {
    position: absolute;
    inset: 0;
    transform: rotate(135deg);
  }

  .loud circle {
    fill: none;
    stroke-width: 7;
    stroke-linecap: round;
  }

  .track {
    stroke: rgba(255, 255, 255, 0.08);
  }

  .value {
    stroke: var(--accent);
    filter: drop-shadow(0 0 6px var(--accent-glow));
    transition: stroke-dasharray 0.3s var(--ease);
  }

  .read {
    display: flex;
    flex-direction: column;
    align-items: center;
    line-height: 1;
  }

  .big {
    font-size: 34px;
    font-weight: 700;
    color: var(--text);
    margin-bottom: 4px;
  }

  .clip {
    position: absolute;
    bottom: -14px;
    left: 50%;
    transform: translateX(-50%);
    white-space: nowrap;
    font-size: 11px;
    color: var(--muted);
  }

  .clip.on {
    color: var(--bad);
  }

  .vol {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .vol-head {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
  }

  .vol-head .num {
    font-size: 15px;
    font-weight: 650;
    color: var(--text);
  }

  .vol input {
    width: 100%;
  }

  .mute {
    align-self: flex-start;
  }

  .mute.on {
    color: var(--bad);
    border-color: color-mix(in oklab, var(--bad) 50%, transparent);
  }

  @media (max-width: 1100px) {
    .stage {
      grid-template-columns: minmax(0, 1fr) auto;
    }

    .vol {
      grid-column: 1 / -1;
    }
  }
</style>
