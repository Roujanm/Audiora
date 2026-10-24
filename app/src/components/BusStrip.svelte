<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import Fader from './Fader.svelte';
  import Meter from './Meter.svelte';

  interface Props {
    index: number;
  }
  let { index }: Props = $props();

  const node = $derived(engine.info?.buses[index]);
  const p = (leaf: string) => `bus/${index}/${leaf}`;
  const limiterOn = $derived(engine.get(p('limiter/enabled')) > 0.5);
  const gr = $derived(engine.gauge(p('limiter/gr_db')));
  const clipping = $derived(engine.holdDb(p('peak_l')) > -0.5 || engine.holdDb(p('peak_r')) > -0.5);
</script>

<section class="strip card" aria-label={node?.name ?? `Bus ${index + 1}`}>
  <header>
    <span class="kind">{index === 0 ? 'MASTER' : `BUS ${index + 1}`}</span>
    <h3 title={node?.name}>{node?.name ?? '—'}</h3>
    <span class="muted rate num">
      {node?.sample_rate ? `${node.sample_rate / 1000} kHz` : ''}
      {node?.low_latency ? ' · low latency' : ''}
    </span>
  </header>

  <div class="btns">
    <button class="tog mute" class:on={engine.get(p('mute')) > 0.5} onclick={() => engine.toggle(p('mute'))} aria-pressed={engine.get(p('mute')) > 0.5}>M</button>
    <button class="tog lim" class:on={limiterOn} onclick={() => engine.toggle(p('limiter/enabled'))} aria-pressed={limiterOn} title="Brick-wall limiter">LIM</button>
  </div>

  <div class="grrow num" class:active={limiterOn && gr > 0.2}>
    {limiterOn ? `GR ${gr.toFixed(1)} dB` : 'limiter off'}
  </div>

  <div class="main">
    <Fader value={engine.get(p('gain_db'))} label="{node?.name ?? 'Bus'} level" onchange={(db) => engine.set(p('gain_db'), db)} />
    <Meter
      levels={[engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))]}
      holds={[engine.holdDb(p('peak_l')), engine.holdDb(p('peak_r'))]}
    />
  </div>

  <div class="clip" class:on={clipping} title="Lights when the peak-hold touches 0 dBFS">
    {clipping ? 'CLIP' : 'OK'}
  </div>
</section>

<style>
  .strip {
    width: 132px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    min-height: 0;
    border-color: #36506f;
    background: linear-gradient(var(--panel), #121821);
  }

  header {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .kind {
    align-self: flex-start;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--accent);
    color: #06101d;
  }

  h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    min-height: 30px;
  }

  .rate {
    font-size: 10px;
  }

  .btns {
    display: flex;
    gap: 4px;
  }

  .tog {
    flex: 1;
    height: 24px;
    border-radius: 4px;
    border: 1px solid var(--line);
    background: var(--panel-2);
    font-size: 11px;
    font-weight: 700;
    color: var(--muted);
  }

  .tog.mute.on {
    background: var(--bad);
    border-color: var(--bad);
    color: #1b0508;
  }

  .tog.lim.on {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
  }

  .grrow {
    font-size: 10px;
    color: var(--faint);
    text-align: center;
    padding: 2px 0;
    border-radius: 3px;
    background: var(--bg);
  }

  .grrow.active {
    color: var(--warn);
  }

  .main {
    display: flex;
    gap: 6px;
    flex: 1;
    min-height: 150px;
  }

  .clip {
    height: 22px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--line);
    background: var(--bg);
    display: grid;
    place-items: center;
    font-size: 10px;
    font-weight: 700;
    color: var(--faint);
  }

  .clip.on {
    background: var(--bad);
    border-color: var(--bad);
    color: #1b0508;
  }
</style>
