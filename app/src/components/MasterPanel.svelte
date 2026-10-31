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

  // the ring: volume as the arc, the live level as a glow inside it
  const R = 70;
  const ARC = 2 * Math.PI * R * 0.75;
  const glow = $derived(Math.max(0, Math.min(1, (peak + 60) / 60)));
</script>

<section class="master panel" class:muted aria-label="Master output">
  <span class="eyebrow">Listening on</span>
  <Popover width={320}>
    {#snippet trigger({ open, toggle })}
      <button class="device" class:open onclick={toggle} aria-haspopup="listbox" title="Choose where you listen">
        <span class="dev-ico"><Icon name={iconFor(device)} size={19} /></span>
        <span class="dev-txt">
          <strong>{switching || device}</strong>
          <small>{switching ? 'Switching…' : master?.auto ? 'Automatic · follows Windows' : 'Chosen output'}</small>
        </span>
        <Icon name="chevron" size={15} />
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

  <div class="ring" style:--glow={glow}>
    <svg viewBox="0 0 180 180" aria-hidden="true">
      <circle class="track" cx="90" cy="90" r={R} stroke-dasharray="{ARC} 999" />
      <circle class="value" cx="90" cy="90" r={R} stroke-dasharray="{ARC * (muted ? 0 : volume / 100)} 999" />
    </svg>
    <div class="core">
      <span class="num vol">{muted ? 'Muted' : volume}<small>{muted ? '' : '%'}</small></span>
      <span class="state" class:hot={clipping}>{clipping ? 'Too loud' : peak > -60 ? 'Playing' : 'Silent'}</span>
    </div>
  </div>

  <div class="slider" style:--fill="{muted ? 0 : volume}%">
    <input type="range" min="0" max="100" step="1" value={volume} aria-label="Master volume" oninput={(e) => engine.set(p('gain_db'), percentToDb(+e.currentTarget.value))} />
    <span class="pct num">{muted ? 'Muted' : `${volume}%`}</span>
  </div>

  <div class="row">
    <button class="mute" class:on={muted} aria-pressed={muted} onclick={() => engine.toggle(p('mute'))}>
      <Icon name={muted ? 'mute' : 'volume'} size={17} />{muted ? 'Unmute' : 'Mute all'}
    </button>
    <div class="lufs" title="Short-term loudness of everything you hear (3 s)">
      <span class="num">{lufs === null ? '—' : lufs.toFixed(0).replace('-', '−')}</span>
      <small>LUFS</small>
    </div>
  </div>
</section>

<style>
  .master {
    position: relative;
    z-index: 5;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 18px;
  }

  .device {
    display: flex;
    align-items: center;
    gap: 11px;
    width: 100%;
    padding: 8px 12px 8px 8px;
    border-radius: 14px;
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
    width: 38px;
    height: 38px;
    border-radius: 11px;
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
    font-size: 14.5px;
    font-weight: 700;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .dev-txt small {
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
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

  /* volume ring with the live level glowing in its centre */
  .ring {
    position: relative;
    /* smaller on short windows so the apps list keeps room */
    width: min(100%, 190px, 24vh);
    aspect-ratio: 1;
    margin: 4px auto 0;
  }

  .ring svg {
    width: 100%;
    height: 100%;
    transform: rotate(135deg);
  }

  .ring circle {
    fill: none;
    stroke-width: 10;
    stroke-linecap: round;
  }

  .track {
    stroke: rgba(255, 255, 255, 0.07);
  }

  .value {
    stroke: var(--accent);
    filter: drop-shadow(0 0 6px var(--accent-glow));
    transition: stroke-dasharray 0.12s;
  }

  .core {
    position: absolute;
    inset: 22%;
    border-radius: 50%;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    background: radial-gradient(circle, color-mix(in oklab, var(--accent) calc(var(--glow) * 26%), transparent), transparent 72%);
  }

  .vol {
    font-size: 34px;
    font-weight: 750;
    letter-spacing: -0.03em;
    line-height: 1;
  }

  .vol small {
    font-size: 15px;
    color: var(--muted);
  }

  .muted .vol {
    font-size: 20px;
    color: var(--bad);
  }

  .state {
    margin-top: 6px;
    font-size: 11px;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.08em;
  }

  .state.hot {
    color: var(--bad);
  }

  .slider {
    position: relative;
    height: 40px;
    border-radius: 12px;
    background: linear-gradient(90deg, color-mix(in oklab, var(--accent) 40%, transparent) var(--fill), rgba(0, 0, 0, 0.35) var(--fill));
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.6), inset 0 0 0 1px rgba(255, 255, 255, 0.05);
  }

  .slider input {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    margin: 0;
    appearance: none;
    background: transparent;
    cursor: pointer;
  }

  .slider input::-webkit-slider-runnable-track {
    height: 100%;
    background: transparent;
  }

  .slider input::-webkit-slider-thumb {
    appearance: none;
    width: 16px;
    height: 40px;
    border-radius: 8px;
    background: linear-gradient(180deg, #ffffff, #c9d0da);
    box-shadow: 0 2px 8px rgba(0, 0, 0, 0.6);
  }

  .pct {
    position: absolute;
    left: 14px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 14px;
    font-weight: 700;
    pointer-events: none;
    display: none;
  }

  /* short windows: the ring gives its room to the apps list, the slider shows the number */
  @media (max-height: 880px) {
    .ring {
      display: none;
    }

    .pct {
      display: block;
    }
  }

  .row {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .mute {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    height: 40px;
    border-radius: 12px;
    font-weight: 650;
    border: 1px solid var(--glass-line);
    background: rgba(255, 255, 255, 0.04);
    color: var(--text);
  }

  .mute.on {
    border-color: color-mix(in oklab, var(--bad) 60%, transparent);
    background: color-mix(in oklab, var(--bad) 26%, transparent);
  }

  .lufs {
    display: flex;
    flex-direction: column;
    align-items: center;
    min-width: 64px;
    padding: 4px 10px;
    border-radius: 12px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--glass-line);
  }

  .lufs .num {
    font-size: 16px;
    font-weight: 700;
  }

  .lufs small {
    font-size: 9.5px;
    letter-spacing: 0.1em;
    color: var(--muted);
  }
</style>
