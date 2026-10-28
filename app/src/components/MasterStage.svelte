<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { dbToPercent, percentToDb } from './VolumeSlider.svelte';
  import { shortName } from '../lib/theme.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';
  import Popover from './Popover.svelte';

  const p = (leaf: string) => `bus/0/${leaf}`;
  const muted = $derived(engine.get(p('mute')) > 0.5);
  const master = $derived(engine.channels?.master_out);
  const device = $derived(shortName(master?.device_name ?? engine.info?.buses[0]?.name ?? 'Output'));
  const peak = $derived(Math.max(engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))));
  const clipping = $derived(engine.holdDb(p('peak_l')) > -0.5 || engine.holdDb(p('peak_r')) > -0.5);
  const lufs = $derived(engine.loudness.s <= -100 ? null : engine.loudness.s);
  const volume = $derived(Math.round(dbToPercent(engine.get(p('gain_db')))));
  const playing = $derived(engine.appGroups.filter((a) => a.active).slice(0, 6));

  // outputs you can listen on: real devices only (a virtual cable or Audiora's own outputs would
  // feed back), and never the output that parks routed apps
  const VIRTUAL = /sonar|voicemeeter|vb-audio|cable|virtual|audiora|steelseries|hi-fi/i;
  const outputs = $derived(
    engine.devices.filter((d) => d.flow === 'render' && !VIRTUAL.test(d.name) && d.id !== engine.channels?.parking?.device_id),
  );
  const iconFor = (name: string) => (/head|ear|buds|airpods|quantum|arctis|hyperx/i.test(name) ? 'headphones' : 'speaker');
  let switching = $state('');

  async function choose(id: string | null, name: string, close: () => void) {
    close();
    const already = id === null ? master?.auto : !master?.auto && master?.device_id === id;
    if (already) return;
    switching = name;
    try {
      await engine.configSet({ master_out: id });
      toast(`Switching to ${name}…`, 'ok');
      await engine.restartEngine();
    } catch (e) {
      switching = '';
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

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
    const soft: number[] = new Array(48).fill(-90);
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
        // a soft, slowly moving wave of the master's spectrum (log frequency, 30 Hz .. 16 kHz)
        const lo = Math.log10(30);
        const hi = Math.log10(16000);
        let k = 0;
        for (let b = 0; b < soft.length; b++) {
          const f = 10 ** (lo + ((hi - lo) * b) / (soft.length - 1));
          while (k < hz.length - 1 && hz[k + 1] < f) k++;
          soft[b] += (spec[k] - soft[b]) * 0.18;
        }
        const accent = getComputedStyle(c).getPropertyValue('--accent').trim() || '#dfe4ec';
        const y = (db: number) => h - Math.max(0, Math.min(1, (db + 84) / 78)) * h * 0.9;
        const x = (b: number) => (b / (soft.length - 1)) * w;
        const path = new Path2D();
        path.moveTo(0, h);
        path.lineTo(x(0), y(soft[0]));
        for (let b = 1; b < soft.length - 1; b++) {
          path.quadraticCurveTo(x(b), y(soft[b]), (x(b) + x(b + 1)) / 2, (y(soft[b]) + y(soft[b + 1])) / 2);
        }
        path.lineTo(w, y(soft[soft.length - 1]));
        path.lineTo(w, h);
        path.closePath();
        const grad = ctx.createLinearGradient(0, 0, 0, h);
        grad.addColorStop(0, accent);
        grad.addColorStop(1, 'rgba(0,0,0,0)');
        ctx.globalAlpha = 0.22;
        ctx.fillStyle = grad;
        ctx.fill(path);
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
  <div class="bg" aria-hidden="true"><canvas bind:this={canvas}></canvas></div>

  <div class="out">
    <span class="eyebrow">Listening on</span>
    <Popover width={340}>
      {#snippet trigger({ open, toggle })}
        <button class="device" class:open onclick={toggle} aria-haspopup="listbox" title="Choose where you listen">
          <span class="dev-ico"><Icon name={iconFor(device)} size={22} /></span>
          <span class="dev-txt">
            <strong>{switching || device}</strong>
            <small>{switching ? 'Switching…' : master?.auto ? 'Automatic · follows Windows' : 'Chosen output'}</small>
          </span>
          <Icon name="chevron" size={16} />
        </button>
      {/snippet}
      {#snippet children({ close })}
        <div class="list" role="listbox" aria-label="Outputs">
          <button class="opt" role="option" aria-selected={master?.auto} class:on={master?.auto} onclick={() => choose(null, 'the Windows default', close)}>
            <span class="o-ico"><Icon name="refresh" size={16} /></span>
            <span class="o-txt"><strong>Automatic</strong><small>Follows the Windows default output</small></span>
            {#if master?.auto}<Icon name="check" size={15} />{/if}
          </button>
          {#each outputs as d (d.id)}
            {@const current = !master?.auto && master?.device_id === d.id}
            <button class="opt" role="option" aria-selected={current} class:on={current} onclick={() => choose(d.id, shortName(d.name), close)}>
              <span class="o-ico"><Icon name={iconFor(d.name)} size={16} /></span>
              <span class="o-txt"><strong>{shortName(d.name)}</strong><small>{d.sample_rate / 1000} kHz{d.default ? ' · Windows default' : ''}</small></span>
              {#if current}<Icon name="check" size={15} />{/if}
            </button>
          {/each}
        </div>
      {/snippet}
    </Popover>
    <div class="apps">
      {#each playing as a (a.name)}
        <span class="app" title={a.name}>{a.name}</span>
      {:else}
        <span class="muted quiet">Nothing is playing</span>
      {/each}
    </div>
  </div>

  <div class="vol">
    <div class="big-slider" style:--fill="{muted ? 0 : volume}%">
      <input
        type="range"
        min="0"
        max="100"
        step="1"
        value={volume}
        aria-label="Master volume"
        oninput={(e) => engine.set(p('gain_db'), percentToDb(+e.currentTarget.value))}
      />
      <span class="pct num">{muted ? 'Muted' : `${volume}%`}</span>
    </div>
    <button class="mute" class:on={muted} aria-pressed={muted} aria-label={muted ? 'Unmute everything' : 'Mute everything'} title={muted ? 'Unmute everything' : 'Mute everything'} onclick={() => engine.toggle(p('mute'))}>
      <Icon name={muted ? 'mute' : 'volume'} size={24} />
    </button>
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
    <span class="clip" class:on={clipping}>{clipping ? 'Too loud' : peak > -60 ? 'Healthy' : 'Silent'}</span>
  </div>
</section>

<style>
  .stage {
    position: relative;
    display: grid;
    grid-template-columns: minmax(240px, 0.9fr) minmax(320px, 1.4fr) auto;
    align-items: center;
    gap: 32px;
    padding: 22px 28px;
    /* above the mixer below, so the output list can open over it */
    z-index: 5;
  }

  /* the spectrum stays inside the rounded card; the output list does not */
  .bg {
    position: absolute;
    inset: 0;
    border-radius: inherit;
    overflow: hidden;
    pointer-events: none;
  }

  canvas {
    width: 100%;
    height: 100%;
  }

  .out,
  .vol,
  .loud {
    position: relative;
  }

  .out {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .device {
    display: flex;
    align-items: center;
    gap: 12px;
    width: 100%;
    padding: 10px 14px 10px 10px;
    border-radius: 16px;
    text-align: left;
    border: 1px solid var(--glass-line);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text);
    transition: border-color 0.15s, background 0.15s;
  }

  .device:hover,
  .device.open {
    border-color: color-mix(in oklab, var(--accent) 45%, var(--glass-line));
    background: rgba(255, 255, 255, 0.07);
  }

  .dev-ico {
    flex: none;
    width: 44px;
    height: 44px;
    border-radius: 13px;
    display: grid;
    place-items: center;
    background: color-mix(in oklab, var(--accent) 16%, transparent);
    color: var(--accent-hi);
  }

  .dev-txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .dev-txt strong {
    font-size: 18px;
    font-weight: 700;
    letter-spacing: -0.01em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dev-txt small {
    font-size: 12px;
    color: var(--muted);
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 4px;
    max-height: 360px;
    overflow-y: auto;
  }

  .opt {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 10px;
    border: 0;
    border-radius: 11px;
    background: none;
    color: var(--text);
    text-align: left;
  }

  .opt:hover {
    background: var(--bg-3);
  }

  .opt.on {
    background: color-mix(in oklab, var(--accent) 14%, transparent);
  }

  .o-ico {
    flex: none;
    width: 32px;
    height: 32px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    background: var(--bg-2);
    color: var(--text-2);
  }

  .o-txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .o-txt strong {
    font-weight: 600;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .o-txt small {
    font-size: 11.5px;
    color: var(--muted);
  }

  .apps {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    margin-top: 4px;
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

  /* ---- the big volume control */
  .vol {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .big-slider {
    position: relative;
    flex: 1;
    height: 64px;
    border-radius: 22px;
    background: rgba(0, 0, 0, 0.35);
    border: 1px solid var(--glass-line);
    box-shadow: inset 0 2px 10px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }

  .big-slider::before {
    content: '';
    position: absolute;
    inset: 0 auto 0 0;
    width: var(--fill);
    background: linear-gradient(90deg, color-mix(in oklab, var(--accent) 35%, transparent), color-mix(in oklab, var(--accent) 80%, transparent));
    box-shadow: 0 0 30px -6px var(--accent-glow);
    transition: width 0.08s linear;
    pointer-events: none;
  }

  .big-slider input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    margin: 0;
    opacity: 0;
    cursor: ew-resize;
  }

  .pct {
    position: absolute;
    left: 20px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 24px;
    font-weight: 700;
    color: var(--text);
    pointer-events: none;
    text-shadow: 0 1px 8px rgba(0, 0, 0, 0.6);
  }

  .mute {
    flex: none;
    width: 64px;
    height: 64px;
    border-radius: 22px;
    display: grid;
    place-items: center;
    border: 1px solid var(--glass-line);
    background: rgba(255, 255, 255, 0.05);
    color: var(--text);
    transition: background 0.15s, border-color 0.15s, color 0.15s;
  }

  .mute:hover {
    background: rgba(255, 255, 255, 0.09);
  }

  .mute.on {
    color: var(--bad);
    border-color: color-mix(in oklab, var(--bad) 55%, transparent);
    background: color-mix(in oklab, var(--bad) 16%, transparent);
  }

  .stage.muted canvas {
    opacity: 0.3;
  }

  /* ---- loudness ring */
  .loud {
    width: 128px;
    height: 128px;
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
    font-size: 32px;
    font-weight: 700;
    color: var(--text);
    margin-bottom: 4px;
  }

  .clip {
    position: absolute;
    bottom: -12px;
    left: 50%;
    transform: translateX(-50%);
    white-space: nowrap;
    font-size: 11px;
    color: var(--muted);
  }

  .clip.on {
    color: var(--bad);
  }

  @media (max-width: 1150px) {
    .stage {
      grid-template-columns: minmax(0, 1fr) auto;
    }

    .vol {
      grid-column: 1 / -1;
      grid-row: 2;
    }
  }
</style>
