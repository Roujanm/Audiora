<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import Slider from './Slider.svelte';

  interface Props {
    strip: number;
  }
  let { strip }: Props = $props();

  const p = (leaf: string) => `strip/${strip}/${leaf}`;
  const g = (leaf: string, d = 0) => engine.get(p(leaf), d);
  const isInput = $derived(engine.info?.strips[strip]?.kind === 'input');
  const masterRate = $derived(engine.info?.buses[0]?.sample_rate ?? 48000);
  const denoiseOk = $derived(masterRate === 48000);

  const gateOpen = $derived(engine.gauge(p('gate/open')) > 0.5);
  const gr = $derived(engine.gauge(p('comp/gr_db')));
  const compOn = $derived(g('comp/enabled') > 0.5);
  const gateOn = $derived(g('gate/enabled') > 0.5);
  const set = (leaf: string) => (v: number) => engine.set(p(leaf), v);
</script>

<div class="dyn">
  {#if isInput}
    <section class="block">
      <div class="head">
        <h2>Noise suppression</h2>
        <button class="sw" class:on={g('denoise') > 0.5} disabled={!denoiseOk} onclick={() => engine.toggle(p('denoise'))} aria-pressed={g('denoise') > 0.5}>
          {g('denoise') > 0.5 ? 'On' : 'Off'}
        </button>
      </div>
      <p class="muted note">
        {#if denoiseOk}
          RNNoise-class neural denoiser for voice. Adds 10 ms of latency to this strip.
        {:else}
          Requires a 48 kHz output; the master device runs at {masterRate / 1000} kHz.
        {/if}
      </p>
    </section>
  {/if}

  <section class="block">
    <div class="head">
      <h2>Gate</h2>
      <span class="led" class:on={gateOn && gateOpen} title={gateOn ? (gateOpen ? 'Open' : 'Closed') : 'Bypassed'}></span>
      <button class="sw" class:on={gateOn} onclick={() => engine.toggle(p('gate/enabled'))} aria-pressed={gateOn}>{gateOn ? 'On' : 'Off'}</button>
    </div>
    <Slider label="Threshold" value={g('gate/threshold_db', -50)} min={-90} max={0} step={0.5} unit=" dB" reset={-50} disabled={!gateOn} onchange={set('gate/threshold_db')} />
    <Slider label="Attack" value={g('gate/attack_ms', 2)} min={0.1} max={100} step={0.1} unit=" ms" reset={2} disabled={!gateOn} onchange={set('gate/attack_ms')} />
    <Slider label="Release" value={g('gate/release_ms', 120)} min={5} max={1500} step={5} digits={0} unit=" ms" reset={120} disabled={!gateOn} onchange={set('gate/release_ms')} />
    <Slider label="Range" value={g('gate/range_db', 60)} min={6} max={90} step={1} digits={0} unit=" dB" reset={60} disabled={!gateOn} onchange={set('gate/range_db')} />
  </section>

  <section class="block">
    <div class="head">
      <h2>Compressor</h2>
      <div class="gr" title="Gain reduction" aria-label="Gain reduction {gr.toFixed(1)} dB">
        <div class="grbar" style:width="{Math.min(gr / 24, 1) * 100}%"></div>
        <span class="num">{compOn ? `−${gr.toFixed(1)} dB` : '—'}</span>
      </div>
      <button class="sw" class:on={compOn} onclick={() => engine.toggle(p('comp/enabled'))} aria-pressed={compOn}>{compOn ? 'On' : 'Off'}</button>
    </div>
    <Slider label="Threshold" value={g('comp/threshold_db', -18)} min={-60} max={0} step={0.5} unit=" dB" reset={-18} disabled={!compOn} onchange={set('comp/threshold_db')} />
    <Slider label="Ratio" value={g('comp/ratio', 3)} min={1} max={20} step={0.1} unit=" :1" reset={3} disabled={!compOn} onchange={set('comp/ratio')} />
    <Slider label="Attack" value={g('comp/attack_ms', 10)} min={0.1} max={200} step={0.1} unit=" ms" reset={10} disabled={!compOn} onchange={set('comp/attack_ms')} />
    <Slider label="Release" value={g('comp/release_ms', 120)} min={10} max={2000} step={5} digits={0} unit=" ms" reset={120} disabled={!compOn} onchange={set('comp/release_ms')} />
    <Slider label="Knee" value={g('comp/knee_db', 6)} min={0} max={24} step={0.5} unit=" dB" reset={6} disabled={!compOn} onchange={set('comp/knee_db')} />
    <Slider label="Make-up" value={g('comp/makeup_db', 0)} min={0} max={24} step={0.5} unit=" dB" reset={0} disabled={!compOn} onchange={set('comp/makeup_db')} />
  </section>
</div>

<style>
  .dyn {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 12px;
    align-items: start;
  }

  .block {
    background: var(--panel-2);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 6px;
  }

  .head h2 {
    margin: 0;
    flex: 1;
  }

  .sw {
    min-width: 46px;
    height: 24px;
    border-radius: 12px;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
  }

  .sw.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #06101d;
  }

  .sw:disabled {
    opacity: 0.4;
  }

  .led {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--panel-3);
    border: 1px solid var(--line);
  }

  .led.on {
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
  }

  .note {
    margin: 0;
    font-size: 12px;
  }

  .gr {
    position: relative;
    width: 110px;
    height: 20px;
    border-radius: 4px;
    background: var(--bg);
    border: 1px solid var(--line);
    overflow: hidden;
  }

  .grbar {
    position: absolute;
    inset: 0 auto 0 0;
    background: rgba(255, 180, 84, 0.45);
  }

  .gr span {
    position: relative;
    display: block;
    text-align: right;
    padding-right: 6px;
    font-size: 11px;
    line-height: 18px;
  }
</style>
