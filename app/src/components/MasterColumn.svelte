<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { periodLabel, rateLabel } from '../lib/theme.ts';
  import AppsDock from './AppsDock.svelte';
  import Fader from './Fader.svelte';
  import Icon from './Icon.svelte';
  import LedMeter from './LedMeter.svelte';
  import Pill from './Pill.svelte';

  const p = (leaf: string) => `bus/0/${leaf}`;
  const node = $derived(engine.info?.buses[0]);
  const limiterOn = $derived(engine.get(p('limiter/enabled')) > 0.5);
  const gr = $derived(limiterOn ? engine.gauge(p('limiter/gr_db')) : 0);
  const clipping = $derived(engine.holdDb(p('peak_l')) > -0.5 || engine.holdDb(p('peak_r')) > -0.5);
  const device = $derived(engine.channels?.master_out.device_name ?? node?.name ?? '');
  const lufs = (v: number) => (v <= -100 ? '—' : v.toFixed(1).replace('-', '−'));
  // streaming platforms normalise to about -14 LUFS, podcasts to -16
  const tone = $derived.by(() => {
    const i = engine.loudness.i;
    if (i <= -100) return '';
    return Math.abs(i + 14) <= 1 || Math.abs(i + 16) <= 1 ? 'good' : i > -9 ? 'hot' : '';
  });
</script>

<div class="col">
  <section class="master panel" aria-label="Master output">
    <i class="topbar"></i>
    <header>
      <span class="badge"><Icon name={device.toLowerCase().includes('head') ? 'headphones' : 'speaker'} size={16} /></span>
      <div class="meta">
        <span class="eyebrow role">Master</span>
        <h3 title={node?.name}>{device || 'Output'}</h3>
        <span class="sub num">{rateLabel(node?.sample_rate)} · {periodLabel(node?.sample_rate, node?.period_frames)}{node?.exclusive ? ' · EXCL' : node?.low_latency ? ' · LL' : ''}</span>
      </div>
    </header>

    <div class="body">
      <div class="faders">
        <Fader value={engine.get(p('gain_db'))} label="Master level" onchange={(db) => engine.set(p('gain_db'), db)} />
        <LedMeter
          thickness={11}
          levels={[engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))]}
          holds={[engine.holdDb(p('peak_l')), engine.holdDb(p('peak_r'))]}
        />
      </div>

      <div class="side">
        <div class="lufs {tone}" role="group" aria-label="Loudness">
          <small>LUFS-I</small>
          <div class="big num">{lufs(engine.loudness.i)}</div>
          <div class="sub num">
            <span title="Momentary (400 ms)">M {lufs(engine.loudness.m)}</span>
            <span title="Short-term (3 s)">S {lufs(engine.loudness.s)}</span>
          </div>
          <button class="rst" title="Restart the loudness measurement" aria-label="Restart loudness measurement" onclick={() => engine.resetLoudness()}>
            <Icon name="refresh" size={12} />
          </button>
        </div>

        <div class="pills">
          <Pill active={engine.get(p('mute')) > 0.5} variant="mute" title="Mute everything" label="Mute master" onclick={() => engine.toggle(p('mute'))}>MUTE</Pill>
          <Pill active={limiterOn} variant="accent" title="Brick-wall limiter at {engine.get(p('limiter/ceiling_db'), -1).toFixed(1)} dB" label="Limiter" onclick={() => engine.toggle(p('limiter/enabled'))}>LIMIT</Pill>
        </div>

        <div class="grbar" class:active={gr > 0.2} title="Limiter gain reduction">
          <div class="fill" style:width="{Math.min(gr / 12, 1) * 100}%"></div>
          <span class="num">{limiterOn ? (gr > 0.05 ? `GR −${gr.toFixed(1)}` : 'GR 0.0') : 'limiter off'}</span>
        </div>
        <div class="clip" class:on={clipping} title="Lights when the peak hold touches 0 dBFS">{clipping ? 'CLIP' : 'OK'}</div>
      </div>
    </div>
  </section>

  <AppsDock />
</div>

<style>
  .col {
    width: 262px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
    min-height: 0;
  }

  .master {
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px;
    border-color: color-mix(in oklab, var(--accent) 35%, transparent);
    background:
      radial-gradient(280px 170px at 50% 0%, color-mix(in oklab, var(--accent) 11%, transparent), transparent 70%),
      linear-gradient(180deg, var(--bg-2), var(--bg-1));
  }

  .topbar {
    position: absolute;
    inset: 0 0 auto 0;
    height: 3px;
    background: var(--accent);
    box-shadow: 0 0 22px 2px var(--accent-glow);
    border-radius: var(--r-lg) var(--r-lg) 0 0;
  }

  header {
    display: flex;
    gap: 10px;
    align-items: center;
    min-width: 0;
  }

  .badge {
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    color: var(--accent-hi);
    background: var(--accent-soft);
    border: 1px solid color-mix(in oklab, var(--accent) 35%, transparent);
  }

  .meta {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .role {
    font-size: 9.5px;
    color: var(--accent-hi);
  }

  h3 {
    font-size: 13px;
    font-weight: 650;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sub {
    font-size: 10px;
    color: var(--faint);
    white-space: nowrap;
  }

  .body {
    display: flex;
    gap: 12px;
    min-height: 0;
  }

  .faders {
    display: flex;
    gap: 8px;
    height: 232px;
    flex: none;
  }

  .side {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 9px;
  }

  .lufs {
    position: relative;
    padding: 9px 10px 8px;
    border-radius: var(--r-md);
    background: var(--bg-0);
    border: 1px solid var(--line);
    box-shadow: inset 0 2px 6px rgba(0, 0, 0, 0.55);
  }

  .lufs small {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--faint);
  }

  .lufs .big {
    font-size: 26px;
    font-weight: 600;
    line-height: 1.1;
    letter-spacing: -0.02em;
  }

  .lufs.good .big {
    color: var(--ok);
  }

  .lufs.hot .big {
    color: var(--warn);
  }

  .lufs .sub {
    display: flex;
    gap: 8px;
    margin-top: 3px;
    color: var(--muted);
  }

  .rst {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    border: 0;
    background: none;
    color: var(--faint);
    padding: 0;
  }

  .rst:hover {
    color: var(--text);
    background: var(--bg-3);
  }

  .pills {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .grbar {
    position: relative;
    height: 20px;
    border-radius: 6px;
    background: var(--bg-0);
    border: 1px solid var(--line);
    overflow: hidden;
    display: grid;
    place-items: center;
    font-size: 10px;
    color: var(--faint);
  }

  .grbar .fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: color-mix(in oklab, var(--warn) 35%, transparent);
  }

  .grbar span {
    position: relative;
  }

  .grbar.active {
    color: var(--warn);
  }

  .clip {
    height: 22px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    font: 700 10px var(--mono);
    letter-spacing: 0.1em;
    color: var(--faint);
    background: var(--bg-0);
    border: 1px solid var(--line);
  }

  .clip.on {
    color: #fff;
    background: var(--bad);
    border-color: var(--bad);
    box-shadow: 0 0 14px var(--bad);
  }

  @media (max-height: 780px) {
    .faders {
      height: 168px;
    }

    .master {
      gap: 8px;
      padding: 12px;
    }

    .lufs .big {
      font-size: 22px;
    }
  }
</style>
