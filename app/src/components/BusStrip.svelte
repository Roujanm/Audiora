<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { periodLabel, rateLabel, shortName } from '../lib/theme.ts';
  import Fader from './Fader.svelte';
  import Icon from './Icon.svelte';
  import LedMeter from './LedMeter.svelte';
  import Pill from './Pill.svelte';

  interface Props {
    index: number;
  }
  let { index }: Props = $props();

  const node = $derived(engine.info?.buses[index]);
  const p = (leaf: string) => `bus/${index}/${leaf}`;
  const limiterOn = $derived(engine.get(p('limiter/enabled')) > 0.5);
  const gr = $derived(limiterOn ? engine.gauge(p('limiter/gr_db')) : 0);
  const clipping = $derived(engine.holdDb(p('peak_l')) > -0.5 || engine.holdDb(p('peak_r')) > -0.5);
  const master = $derived(index === 0);
  const name = $derived(node ? shortName(node.name) : '—');
  const accent = 'var(--accent)';
</script>

<section class="bus panel" class:master style:--c={accent} aria-label={node?.name ?? `Bus ${index + 1}`}>
  <i class="topbar"></i>

  <header>
    <span class="badge"><Icon name={name.toLowerCase().includes('head') ? 'headphones' : 'speaker'} size={15} /></span>
    <div class="meta">
      <span class="role eyebrow">{master ? 'Master output' : `Output ${index + 1}`}</span>
      <h3 title={node?.name}>{name}</h3>
      <span class="sub num">
        {rateLabel(node?.sample_rate)} · {periodLabel(node?.sample_rate, node?.period_frames)}{node?.exclusive ? ' · EXCL' : node?.low_latency ? ' · LL' : ''}
      </span>
    </div>
  </header>

  <div class="pills">
    <Pill active={engine.get(p('mute')) > 0.5} variant="mute" title="Mute" label="Mute {name}" onclick={() => engine.toggle(p('mute'))}>MUTE</Pill>
    <Pill active={limiterOn} variant="accent" title="Brick-wall limiter" label="Limiter {name}" onclick={() => engine.toggle(p('limiter/enabled'))}>LIMIT</Pill>
  </div>

  <div class="grbar" class:active={gr > 0.2} title="Limiter gain reduction">
    <div class="fill" style:width="{Math.min(gr / 12, 1) * 100}%"></div>
    <span class="num">{limiterOn ? (gr > 0.05 ? `GR −${gr.toFixed(1)} dB` : 'GR 0.0 dB') : 'limiter off'}</span>
  </div>

  <div class="main">
    <Fader value={engine.get(p('gain_db'))} label="{name} level" onchange={(db) => engine.set(p('gain_db'), db)} />
    <LedMeter
      thickness={master ? 11 : 9}
      levels={[engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))]}
      holds={[engine.holdDb(p('peak_l')), engine.holdDb(p('peak_r'))]}
    />
  </div>

  <div class="clip" class:on={clipping} title="Lights when the peak hold touches 0 dBFS">{clipping ? 'CLIP' : 'OK'}</div>
</section>

<style>
  .bus {
    width: 156px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 14px 12px;
    min-height: 0;
    overflow: hidden;
  }

  .bus.master {
    width: 176px;
    border-color: color-mix(in oklab, var(--accent) 35%, transparent);
    background:
      radial-gradient(260px 160px at 50% 0%, color-mix(in oklab, var(--accent) 11%, transparent), transparent 70%),
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
    gap: 9px;
    align-items: flex-start;
  }

  .badge {
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    color: var(--accent-hi);
    background: var(--accent-soft);
    border: 1px solid color-mix(in oklab, var(--accent) 35%, transparent);
  }

  .meta {
    min-width: 0;
    height: 62px;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  .role {
    font-size: 9px;
    color: var(--accent-hi);
  }

  h3 {
    font-size: 12.5px;
    font-weight: 650;
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }

  .sub {
    font-size: 10px;
    color: var(--faint);
    white-space: nowrap;
  }

  .pills {
    display: flex;
    gap: 5px;
  }

  .grbar {
    position: relative;
    height: 22px;
    border-radius: 6px;
    background: var(--bg-0);
    border: 1px solid var(--line);
    overflow: hidden;
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.6);
  }

  .grbar .fill {
    position: absolute;
    inset: 0 auto 0 0;
    background: linear-gradient(90deg, color-mix(in oklab, var(--warn) 30%, transparent), var(--warn));
    opacity: 0.6;
    transition: width 0.06s linear;
  }

  .grbar span {
    position: relative;
    display: block;
    text-align: center;
    font-size: 10px;
    line-height: 20px;
    color: var(--faint);
  }

  .grbar.active span {
    color: var(--text);
  }

  .main {
    display: flex;
    gap: 8px;
    flex: 1;
    min-height: 170px;
    justify-content: center;
  }

  .clip {
    height: 24px;
    border-radius: 6px;
    border: 1px solid var(--line);
    background: var(--bg-0);
    display: grid;
    place-items: center;
    font: 700 10px var(--font);
    letter-spacing: 0.1em;
    color: var(--faint);
  }

  .clip.on {
    background: var(--bad);
    border-color: transparent;
    color: #230408;
    box-shadow: 0 0 16px var(--bad);
  }

  @media (max-height: 780px) {
    .bus {
      gap: 8px;
      padding: 12px 12px 10px;
    }

    .main {
      min-height: 120px;
    }
  }
</style>
