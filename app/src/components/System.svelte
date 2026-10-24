<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { periodLabel, rateLabel, shortName } from '../lib/theme.ts';
  import Icon from './Icon.svelte';

  const s = $derived(engine.stats);
  const total = $derived(s.underruns + s.drift_drops + s.capture_drops + s.device_errors);
  const health = $derived(total === 0 ? 'good' : total <= 6 ? 'fair' : 'poor');
  const lic = $derived(engine.license);

  const nodes = $derived([
    ...(engine.info?.strips.filter((n) => n.kind === 'input') ?? []).map((n) => ({ ...n, role: 'Input' })),
    ...(engine.info?.buses ?? []).map((n, i) => ({ ...n, role: i === 0 ? 'Master output' : `Output ${i + 1}` })),
  ]);

  const ms = (n: { sample_rate: number | null; period_frames: number | null }) =>
    n.sample_rate && n.period_frames ? (n.period_frames / n.sample_rate) * 1000 : 0;
  const maxMs = $derived(Math.max(20, ...nodes.map(ms)));

  const renders = $derived(engine.devices.filter((d) => d.flow === 'render'));
  const captures = $derived(engine.devices.filter((d) => d.flow === 'capture'));

  const stat = $derived([
    { label: 'Underruns', value: s.underruns, hint: 'A device ran out of audio' },
    { label: 'Drift drops', value: s.drift_drops, hint: 'Two clocks diverged beyond what the resampler can trim' },
    { label: 'Dropped blocks', value: s.capture_drops, hint: 'A buffer was full' },
    { label: 'Device errors', value: s.device_errors, hint: 'A device vanished; the engine reconnects by itself' },
  ]);

  const R = 46;
  const C = 2 * Math.PI * R;
</script>

<div class="sys">
  <section class="panel hero" class:good={health === 'good'} class:fair={health === 'fair'} class:poor={health === 'poor'}>
    <div class="ring" aria-hidden="true">
      <svg viewBox="0 0 110 110" width="116" height="116">
        <circle cx="55" cy="55" r={R} class="bg" />
        <circle cx="55" cy="55" r={R} class="fg" stroke-dasharray="{C * (health === 'good' ? 1 : health === 'fair' ? 0.66 : 0.33)} {C}" transform="rotate(-90 55 55)" />
      </svg>
      <span class="ico"><Icon name={health === 'good' ? 'check' : 'alert'} size={30} stroke={2.2} /></span>
    </div>
    <div class="txt">
      <h2>{health === 'good' ? 'Audio is running clean' : health === 'fair' ? 'A few glitches so far' : 'Audio is glitching'}</h2>
      <p class="muted">
        {s.callbacks.toLocaleString()} audio blocks processed · {total} glitch event{total === 1 ? '' : 's'}.
        {#if total > 0 && total <= 6}A handful at start-up is normal.{/if}
      </p>
    </div>
    <div class="stats">
      {#each stat as t (t.label)}
        <div class="stat" title={t.hint}>
          <span class="num v" class:warn={t.value > 0}>{t.value}</span>
          <span class="l">{t.label}</span>
        </div>
      {/each}
    </div>
  </section>

  <section class="panel pad">
    <h2 class="eyebrow">Active devices</h2>
    <div class="devs">
      {#each nodes as n, i (i)}
        <div class="dev">
          <span class="role">{n.role}</span>
          <span class="name" title={n.name}>{shortName(n.name)}</span>
          <span class="num rate">{rateLabel(n.sample_rate)}</span>
          <div class="bar" title="Device period">
            <i style:width="{(ms(n) / maxMs) * 100}%"></i>
          </div>
          <span class="num per">
            {periodLabel(n.sample_rate, n.period_frames)}
            {#if n.exclusive}<span class="chip accent">EXCL {n.bit_depth}-bit</span>{:else if n.low_latency}<span class="chip accent">LL</span>{/if}
          </span>
        </div>
      {/each}
    </div>
  </section>

  <section class="panel pad lic">
    <h2 class="eyebrow">Licence</h2>
    {#if lic}
      {#if lic.state === 'valid'}
        <p><span class="chip ok">Valid</span> <b>{lic.edition}</b> edition · {lic.issued_to}</p>
      {:else if lic.state === 'dev-build'}
        <p><span class="chip warn">Development build</span> <span class="muted">Licence checks are off.</span></p>
      {:else if lic.state === 'none'}
        <p><span class="chip">Play edition</span> <span class="muted">No licence file found.</span></p>
      {:else}
        <p><span class="chip bad">Invalid</span> {lic.error}</p>
      {/if}
    {/if}
  </section>

  <section class="panel pad wide">
    <h2 class="eyebrow">Windows audio devices</h2>
    <div class="cols">
      <div>
        <h3>Playback <span class="chip">{renders.length}</span></h3>
        <ul>
          {#each renders as d (d.id)}
            <li><Icon name="speaker" size={13} /><span class="n" title={d.name}>{d.name}</span><span class="num muted">{d.sample_rate / 1000} kHz · {d.channels} ch</span>{#if d.default}<span class="chip accent">default</span>{/if}</li>
          {/each}
        </ul>
      </div>
      <div>
        <h3>Recording <span class="chip">{captures.length}</span></h3>
        <ul>
          {#each captures as d (d.id)}
            <li><Icon name="mic" size={13} /><span class="n" title={d.name}>{d.name}</span><span class="num muted">{d.sample_rate / 1000} kHz · {d.channels} ch</span>{#if d.default}<span class="chip accent">default</span>{/if}</li>
          {/each}
        </ul>
      </div>
    </div>
  </section>
</div>

<style>
  .sys {
    display: grid;
    grid-template-columns: 1fr 340px;
    gap: 16px;
    max-width: 1180px;
    align-items: start;
  }

  .hero {
    grid-column: 1 / -1;
    display: flex;
    align-items: center;
    gap: 26px;
    padding: 22px 28px;
    --h: var(--ok);
  }

  .hero.fair {
    --h: var(--warn);
  }

  .hero.poor {
    --h: var(--bad);
  }

  .hero {
    background:
      radial-gradient(520px 160px at 0% 50%, color-mix(in oklab, var(--h) 11%, transparent), transparent 70%),
      linear-gradient(180deg, var(--bg-2), var(--bg-1));
  }

  .ring {
    position: relative;
    flex: none;
    width: 116px;
    height: 116px;
  }

  .ring svg {
    display: block;
  }

  .bg {
    fill: none;
    stroke: var(--bg-4);
    stroke-width: 7;
  }

  .fg {
    fill: none;
    stroke: var(--h);
    stroke-width: 7;
    stroke-linecap: round;
    filter: drop-shadow(0 0 7px var(--h));
    transition: stroke-dasharray 0.4s var(--ease);
  }

  .ico {
    position: absolute;
    inset: 0;
    display: grid;
    place-items: center;
    color: var(--h);
  }

  .txt {
    flex: 1;
    min-width: 0;
  }

  .txt h2 {
    font-size: 19px;
    font-weight: 700;
    letter-spacing: -0.01em;
    margin-bottom: 4px;
  }

  .stats {
    display: grid;
    grid-template-columns: repeat(4, auto);
    gap: 4px 26px;
  }

  .stat {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .v {
    font-size: 26px;
    font-weight: 500;
  }

  .v.warn {
    color: var(--warn);
  }

  .stat .l {
    font-size: 10.5px;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .devs {
    display: flex;
    flex-direction: column;
    margin-top: 10px;
  }

  .dev {
    display: grid;
    grid-template-columns: 112px minmax(120px, 1.4fr) 62px 1fr 120px;
    align-items: center;
    gap: 14px;
    padding: 11px 4px;
    border-top: 1px solid var(--line);
    font-size: 12.5px;
  }

  .dev:first-child {
    border-top: 0;
  }

  .role {
    font-size: 10.5px;
    font-weight: 650;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .name {
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rate {
    color: var(--text-2);
  }

  .bar {
    height: 6px;
    border-radius: 3px;
    background: var(--bg-0);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.6);
    overflow: hidden;
  }

  .bar i {
    display: block;
    height: 100%;
    border-radius: 3px;
    background: linear-gradient(90deg, var(--accent-lo), var(--accent));
    box-shadow: 0 0 10px var(--accent-glow);
  }

  .per {
    display: flex;
    gap: 8px;
    align-items: center;
    justify-content: flex-end;
  }

  .lic p {
    margin-top: 12px;
    display: flex;
    gap: 10px;
    align-items: center;
    flex-wrap: wrap;
  }

  .wide {
    grid-column: 1 / -1;
  }

  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 26px;
    margin-top: 10px;
  }

  h3 {
    display: flex;
    align-items: center;
    gap: 8px;
    margin: 0 0 8px;
    font-size: 12px;
    font-weight: 650;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    max-height: 300px;
    overflow: auto;
  }

  li {
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 6px 4px;
    border-top: 1px solid var(--line);
    font-size: 12px;
    min-width: 0;
    color: var(--text-2);
  }

  li:first-child {
    border-top: 0;
  }

  li .n {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  @media (max-width: 1100px) {
    .sys {
      grid-template-columns: 1fr;
    }
    .dev {
      grid-template-columns: 96px 1fr 60px 100px;
    }
    .bar {
      display: none;
    }
  }
</style>
