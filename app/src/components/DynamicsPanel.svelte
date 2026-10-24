<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { gateGainDb, outputDb } from '../lib/dynamics.ts';
  import CurveGraph from './CurveGraph.svelte';
  import Icon from './Icon.svelte';
  import Knob from './Knob.svelte';
  import Switch from './Switch.svelte';

  interface Props {
    strip: number;
  }
  let { strip }: Props = $props();

  const p = (leaf: string) => `strip/${strip}/${leaf}`;
  const g = (leaf: string, d = 0) => engine.get(p(leaf), d);
  const set = (leaf: string) => (v: number) => engine.set(p(leaf), v);
  const flag = (leaf: string) => (v: boolean) => engine.set(p(leaf), v);

  const isInput = $derived(engine.info?.strips[strip]?.kind === 'input');
  const masterRate = $derived(engine.info?.buses[0]?.sample_rate ?? 48000);
  const denoiseOk = $derived(masterRate === 48000);

  const gateOn = $derived(g('gate/enabled') > 0.5);
  const gateOpen = $derived(engine.gauge(p('gate/open')) > 0.5);
  const compOn = $derived(g('comp/enabled') > 0.5);
  const gr = $derived(compOn ? engine.gauge(p('comp/gr_db')) : 0);
  const denoise = $derived(g('denoise') > 0.5);

  const comp = $derived({ thresholdDb: g('comp/threshold_db', -18), ratio: g('comp/ratio', 3), kneeDb: g('comp/knee_db', 6) });
  const makeup = $derived(g('comp/makeup_db'));

  const ms = (v: number) => (v >= 100 ? `${Math.round(v)} ms` : `${v.toFixed(v < 10 ? 1 : 0)} ms`);
  const db = (v: number) => `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toFixed(1)} dB`;
</script>

<div class="dyn">
  {#if isInput}
    <section class="card panel" class:off={!denoise}>
      <header>
        <span class="ico"><Icon name="bolt" size={16} /></span>
        <h2 class="eyebrow">Noise suppression</h2>
        <Switch checked={denoise} label="Noise suppression" disabled={!denoiseOk} onchange={flag('denoise')} />
      </header>
      <p class="muted copy">
        {#if denoiseOk}
          Neural voice denoiser (RNNoise class). Removes fans, keyboard and room noise from this input. Adds 10 ms.
        {:else}
          Needs a 48 kHz master output. The master runs at {masterRate / 1000} kHz right now.
        {/if}
      </p>
    </section>
  {/if}

  <section class="card panel" class:off={!gateOn}>
    <header>
      <span class="led" class:on={gateOn && gateOpen} title={gateOn ? (gateOpen ? 'Open' : 'Closed') : 'Bypassed'}></span>
      <h2 class="eyebrow">Gate</h2>
      <Switch checked={gateOn} label="Gate" onchange={flag('gate/enabled')} />
    </header>
    <div class="body">
      <CurveGraph
        label="Gate transfer curve"
        thresholdDb={g('gate/threshold_db', -50)}
        active={gateOn}
        color="var(--ok)"
        fn={(i) => i + gateGainDb(i, g('gate/threshold_db', -50), g('gate/range_db', 60))}
      />
      <div class="knobs">
        <Knob label="Thresh" min={-90} max={0} step={0.5} reset={-50} color="var(--ok)" disabled={!gateOn} value={g('gate/threshold_db', -50)} format={db} onchange={set('gate/threshold_db')} />
        <Knob label="Range" min={6} max={90} step={1} reset={60} color="var(--ok)" disabled={!gateOn} value={g('gate/range_db', 60)} format={(v) => `${Math.round(v)} dB`} onchange={set('gate/range_db')} />
        <Knob label="Attack" min={0.1} max={100} scale="log" step={0.1} reset={2} color="var(--ok)" disabled={!gateOn} value={g('gate/attack_ms', 2)} format={ms} onchange={set('gate/attack_ms')} />
        <Knob label="Release" min={5} max={1500} scale="log" step={1} reset={120} color="var(--ok)" disabled={!gateOn} value={g('gate/release_ms', 120)} format={ms} onchange={set('gate/release_ms')} />
      </div>
    </div>
  </section>

  <section class="card panel wide" class:off={!compOn}>
    <header>
      <span class="ico warn"><Icon name="wave" size={16} /></span>
      <h2 class="eyebrow">Compressor</h2>
      <div class="gr" role="img" aria-label="Gain reduction {gr.toFixed(1)} dB">
        <div class="grfill" style:width="{Math.min(gr / 18, 1) * 100}%"></div>
        <span class="num">{compOn ? `−${gr.toFixed(1)} dB` : 'off'}</span>
      </div>
      <Switch checked={compOn} label="Compressor" onchange={flag('comp/enabled')} />
    </header>
    <div class="body">
      <CurveGraph
        label="Compressor transfer curve"
        width={210}
        height={170}
        thresholdDb={comp.thresholdDb}
        active={compOn}
        color="var(--warn)"
        fn={(i) => outputDb(i, comp, makeup)}
      />
      <div class="knobs two">
        <Knob label="Thresh" min={-60} max={0} step={0.5} reset={-18} color="var(--warn)" disabled={!compOn} value={comp.thresholdDb} format={db} onchange={set('comp/threshold_db')} />
        <Knob label="Ratio" min={1} max={20} scale="log" step={0.1} reset={3} color="var(--warn)" disabled={!compOn} value={comp.ratio} format={(v) => `${v.toFixed(1)}:1`} onchange={set('comp/ratio')} />
        <Knob label="Knee" min={0} max={24} step={0.5} reset={6} color="var(--warn)" disabled={!compOn} value={comp.kneeDb} format={(v) => `${v.toFixed(1)} dB`} onchange={set('comp/knee_db')} />
        <Knob label="Attack" min={0.1} max={200} scale="log" step={0.1} reset={10} color="var(--warn)" disabled={!compOn} value={g('comp/attack_ms', 10)} format={ms} onchange={set('comp/attack_ms')} />
        <Knob label="Release" min={10} max={2000} scale="log" step={1} reset={120} color="var(--warn)" disabled={!compOn} value={g('comp/release_ms', 120)} format={ms} onchange={set('comp/release_ms')} />
        <Knob label="Make-up" min={0} max={24} step={0.5} reset={0} color="var(--warn)" disabled={!compOn} value={makeup} format={db} onchange={set('comp/makeup_db')} />
      </div>
    </div>
  </section>
</div>

<style>
  .dyn {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(420px, 1fr));
    gap: 16px;
    align-items: start;
  }

  .wide {
    grid-column: 1 / -1;
  }

  .card {
    padding: 16px 18px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    transition: opacity 0.2s;
  }

  .card.off .body,
  .card.off .copy {
    opacity: 0.62;
  }

  header {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  header h2 {
    flex: 1;
  }

  .ico {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 8px;
    color: var(--ok);
    background: color-mix(in oklab, var(--ok) 14%, transparent);
  }

  .ico.warn {
    color: var(--warn);
    background: color-mix(in oklab, var(--warn) 14%, transparent);
  }

  .led {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--bg-4);
    box-shadow: inset 0 1px 2px rgba(0, 0, 0, 0.6);
  }

  .led.on {
    background: var(--ok);
    box-shadow: 0 0 10px var(--ok);
  }

  .copy {
    font-size: 12.5px;
    max-width: 52ch;
  }

  .body {
    display: flex;
    gap: 18px;
    align-items: center;
    flex-wrap: wrap;
  }

  .knobs {
    flex: 1;
    min-width: 230px;
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 14px 8px;
  }

  .knobs.two {
    grid-template-columns: repeat(3, 1fr);
  }

  .gr {
    position: relative;
    width: 130px;
    height: 24px;
    border-radius: 7px;
    background: var(--bg-0);
    border: 1px solid var(--line);
    overflow: hidden;
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.6);
  }

  .grfill {
    position: absolute;
    inset: 0 auto 0 0;
    background: linear-gradient(90deg, color-mix(in oklab, var(--warn) 25%, transparent), var(--warn));
    opacity: 0.65;
    transition: width 0.06s linear;
  }

  .gr span {
    position: relative;
    display: block;
    text-align: right;
    padding-right: 8px;
    font-size: 11px;
    line-height: 22px;
  }
</style>
