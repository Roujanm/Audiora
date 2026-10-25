<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { shortName } from '../lib/theme.ts';
  import AppsDock from './AppsDock.svelte';
  import Icon from './Icon.svelte';
  import VolumeKnob from './VolumeKnob.svelte';

  const p = (leaf: string) => `bus/0/${leaf}`;
  const muted = $derived(engine.get(p('mute')) > 0.5);
  const device = $derived(shortName(engine.channels?.master_out.device_name ?? engine.info?.buses[0]?.name ?? 'Output'));
  const levels = $derived([engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))]);
  const holds = $derived([engine.holdDb(p('peak_l')), engine.holdDb(p('peak_r'))]);
  const clipping = $derived(engine.holdDb(p('peak_l')) > -0.5 || engine.holdDb(p('peak_r')) > -0.5);
  const lufs = $derived(engine.loudness.s <= -100 ? '—' : engine.loudness.s.toFixed(0).replace('-', '−'));
</script>

<div class="side">
  <section class="master panel" aria-label="Master">
    <header>
      <span class="badge"><Icon name={device.toLowerCase().includes('head') ? 'headphones' : 'speaker'} size={17} /></span>
      <span class="txt">
        <strong>Master</strong>
        <small title={device}>{device}</small>
      </span>
    </header>

    <div class="body">
      <VolumeKnob db={engine.get(p('gain_db'))} {levels} {holds} {muted} size={132} label="Master volume" onchange={(db) => engine.set(p('gain_db'), db)} />
      <div class="info">
        <button class="round" class:on={muted} aria-pressed={muted} title={muted ? 'Unmute everything' : 'Mute everything'} aria-label="Mute master" onclick={() => engine.toggle(p('mute'))}>
          <Icon name={muted ? 'mute' : 'volume'} size={17} />
        </button>
        <div class="stat" title="Short-term loudness (3 s)">
          <span class="num">{lufs}</span>
          <small>LUFS</small>
        </div>
        <span class="clip" class:on={clipping} title={clipping ? 'Too loud: the output touched 0 dBFS' : 'Output level is safe'}></span>
      </div>
    </div>
  </section>

  <AppsDock />
</div>

<style>
  .side {
    width: 248px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }

  @media (max-width: 1240px) {
    .side {
      width: 218px;
    }
  }

  .master {
    flex: 0 0 46%;
    min-height: 230px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 12px 14px 14px;
    background:
      radial-gradient(240px 140px at 50% 0%, color-mix(in oklab, var(--accent) 12%, transparent), transparent 75%),
      linear-gradient(180deg, var(--bg-2), var(--bg-1));
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .badge {
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    color: var(--accent-hi);
    background: var(--accent-soft);
  }

  .txt {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  strong {
    font-size: 14.5px;
    font-weight: 700;
  }

  small {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .body {
    flex: 1;
    min-height: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
  }

  .info {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 18px;
  }

  .round {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    padding: 0;
    border: 1px solid var(--line-2);
    background: var(--bg-2);
    color: var(--text-2);
  }

  .round:hover {
    color: var(--text);
    border-color: var(--line-3);
  }

  .round.on {
    color: #fff;
    background: var(--bad);
    border-color: transparent;
  }

  .stat {
    display: flex;
    align-items: baseline;
    gap: 5px;
  }

  .stat .num {
    font-size: 17px;
    font-weight: 600;
  }

  .stat small {
    font-size: 9.5px;
    letter-spacing: 0.1em;
    color: var(--faint);
  }

  .clip {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: color-mix(in oklab, var(--ok) 70%, transparent);
    box-shadow: 0 0 8px color-mix(in oklab, var(--ok) 60%, transparent);
  }

  .clip.on {
    background: var(--bad);
    box-shadow: 0 0 12px var(--bad);
  }
</style>
