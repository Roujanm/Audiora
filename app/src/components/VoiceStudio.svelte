<script lang="ts">
  import { channelMeta } from '../lib/channels.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import type { ChannelEntry } from '../lib/protocol.ts';
  import { shortName } from '../lib/theme.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import EqEditor from './EqEditor.svelte';
  import Icon from './Icon.svelte';
  import Knob from './Knob.svelte';
  import MicCheck from './MicCheck.svelte';
  import Popover from './Popover.svelte';
  import PresetPicker from './PresetPicker.svelte';
  import Segmented from './Segmented.svelte';
  import Switch from './Switch.svelte';

  interface Props {
    ch: ChannelEntry;
  }
  let { ch }: Props = $props();

  const meta = $derived(channelMeta('mic')!);
  const p = (leaf: string) => `strip/${ch.strip}/${leaf}`;
  const g = (leaf: string, d = 0) => engine.get(p(leaf), d);
  const on = (leaf: string) => g(leaf) > 0.5;
  const set = (leaf: string) => (v: number | boolean) => engine.set(p(leaf), v);

  const db = (v: number) => `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toFixed(1)} dB`;
  const ms = (v: number) => (v >= 100 ? `${Math.round(v)} ms` : `${v.toFixed(1)} ms`);
  const pct = (v: number) => `${Math.round(v * 100)}%`;
  const grBar = (gr: number) => `${Math.max(0, Math.min(100, (gr / 12) * 100))}%`;

  const muted = $derived(on('mute'));
  const monitoring = $derived((g('bus_mask') & 1) === 1);
  const gateOpen = $derived(engine.gauge(p('gate/open')) > 0.5);
  const deessGr = $derived(on('deess/enabled') ? engine.gauge(p('deess/gr_db')) : 0);
  const compGr = $derived(on('comp/enabled') ? engine.gauge(p('comp/gr_db')) : 0);
  const limitGr = $derived(on('limiter/enabled') ? engine.gauge(p('limiter/gr_db')) : 0);
  const smartGain = $derived(on('smart/enabled') ? engine.gauge(p('smart/gain_db')) : 0);
  const LOWCUTS = [60, 80, 100, 120, 150];
  const lowcut = $derived(LOWCUTS.reduce((b, f) => (Math.abs(f - g('lowcut/freq', 80)) < Math.abs(b - g('lowcut/freq', 80)) ? f : b)));

  // Windows' own input level for this microphone (the analogue gain before Audiora)
  let hwLevel = $state<number | null>(null);
  async function readHw() {
    try {
      hwLevel = (await engine.call('mic_input')).level as number;
    } catch {
      hwLevel = null;
    }
  }
  function setHw(v: number) {
    hwLevel = v;
    engine.call('mic_input', { level: v }).catch((e) => toast(String(e), 'error'));
  }
  $effect(() => {
    void ch.device_id;
    void readHw();
  });

  let saveName = $state('');
  async function save(close: () => void) {
    const n = saveName.trim();
    if (!n) return;
    try {
      await engine.saveChannelPreset('mic', n);
      toast(`Saved “${n}”`, 'ok');
      saveName = '';
      close();
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }
</script>

<div class="voice" style:--c={meta.color}>
  <header class="hero">
    <div class="id">
      <span class="badge"><Icon name="mic" size={22} /></span>
      <div>
        <p class="eyebrow">Voice</p>
        <h1 title={ch.device_name ?? ''}>{ch.online ? shortName(ch.device_name ?? 'Microphone') : 'No microphone found'}</h1>
      </div>
    </div>
    <div class="actions">
      <PresetPicker channel="mic" color={meta.color} />
      <Popover width={260}>
        {#snippet trigger({ open, toggle })}
          <button class="btn icon-btn" class:open title="Save as a new preset" aria-label="Save as a new preset" onclick={toggle}><Icon name="save" size={16} /></button>
        {/snippet}
        {#snippet children({ close })}
          <form
            class="save"
            onsubmit={(e) => {
              e.preventDefault();
              void save(close);
            }}
          >
            <p class="eyebrow">Save as preset</p>
            <input type="text" bind:value={saveName} placeholder="Name" maxlength="40" aria-label="Preset name" />
            <button class="btn sm primary" type="submit" disabled={!saveName.trim()}>Save</button>
          </form>
        {/snippet}
      </Popover>
      <button class="mute" class:on={muted} aria-pressed={muted} onclick={() => engine.set(p('mute'), !muted)} title="Mutes the microphone in Windows: no program hears you">
        <Icon name={muted ? 'mute' : 'mic'} size={17} />{muted ? 'Muted everywhere' : 'Live'}
      </button>
    </div>
  </header>


  <EqEditor strip={ch.strip} />

  <div class="rack">
    <!-- 1. input: Windows' analogue gain, then Audiora's digital gain -->
    <section class="panel mod" aria-label="Input">
      <div class="mod-head">
        <span class="mod-title"><Icon name="volume" size={15} />Input</span>
      </div>
      <div class="knobs">
        {#if hwLevel !== null}
          <Knob label="Mic level" min={0} max={1} step={0.01} reset={1} size={64} color={meta.color} value={hwLevel} format={pct} onchange={setHw} />
        {/if}
        <Knob label="Gain" min={0} max={48} step={0.5} reset={0} size={64} color={meta.color} value={g('boost_db')} format={db} onchange={set('boost_db')} />
      </div>
      <p class="note">Mic level is Windows' own input gain; Gain is added in Audiora.</p>
    </section>

    <section class="panel mod" class:off={!on('lowcut/enabled')} aria-label="Low cut">
      <div class="mod-head">
        <span class="mod-title"><Icon name="wave" size={15} />Low cut</span>
        <Switch label="Low cut" checked={on('lowcut/enabled')} onchange={set('lowcut/enabled')} />
      </div>
      <Segmented size="sm" label="Low cut frequency" value={String(lowcut)} options={LOWCUTS.map((f) => ({ id: String(f), label: `${f}` }))} onchange={(id) => engine.set(p('lowcut/freq'), +id)} />
      <p class="note">Hz · removes rumble, desk bumps and breath pops.</p>
    </section>

    <section class="panel mod" class:off={!on('denoise')} aria-label="ClearCast">
      <div class="mod-head">
        <span class="mod-title"><Icon name="bolt" size={15} />ClearCast</span>
        <Switch label="ClearCast" checked={on('denoise')} onchange={set('denoise')} />
      </div>
      <div class="knobs">
        <Knob label="Strength" min={0} max={1} step={0.01} reset={1} size={64} color={meta.color} disabled={!on('denoise')} value={g('denoise_mix', 1)} format={pct} onchange={set('denoise_mix')} />
      </div>
      <p class="note">AI noise removal: keyboard, fans, room hiss.</p>
    </section>

    <section class="panel mod" class:off={!on('gate/enabled')} aria-label="Noise gate">
      <div class="mod-head">
        <span class="mod-title"><span class="led" class:lit={on('gate/enabled') && gateOpen}></span>Gate</span>
        <Switch label="Noise gate" checked={on('gate/enabled')} onchange={set('gate/enabled')} />
      </div>
      <div class="knobs">
        <Knob label="Thresh" min={-90} max={0} step={0.5} reset={-50} size={52} color="var(--ok)" disabled={!on('gate/enabled')} value={g('gate/threshold_db', -50)} format={db} onchange={set('gate/threshold_db')} />
        <Knob label="Range" min={6} max={90} step={1} reset={15} size={52} color="var(--ok)" disabled={!on('gate/enabled')} value={g('gate/range_db', 15)} format={(v) => `${Math.round(v)} dB`} onchange={set('gate/range_db')} />
        <Knob label="Hold" min={0} max={1000} step={5} reset={200} size={52} color="var(--ok)" disabled={!on('gate/enabled')} value={g('gate/hold_ms', 200)} format={ms} onchange={set('gate/hold_ms')} />
        <Knob label="Release" min={5} max={1500} scale="log" step={1} reset={200} size={52} color="var(--ok)" disabled={!on('gate/enabled')} value={g('gate/release_ms', 200)} format={ms} onchange={set('gate/release_ms')} />
      </div>
    </section>

    <section class="panel mod" class:off={!on('deess/enabled')} aria-label="De-esser">
      <div class="mod-head">
        <span class="mod-title"><Icon name="chat" size={15} />De-esser</span>
        <Switch label="De-esser" checked={on('deess/enabled')} onchange={set('deess/enabled')} />
      </div>
      <div class="knobs">
        <Knob label="Thresh" min={-60} max={0} step={0.5} reset={-30} size={64} color="var(--warn)" disabled={!on('deess/enabled')} value={g('deess/threshold_db', -30)} format={db} onchange={set('deess/threshold_db')} />
      </div>
      <div class="gr" aria-label="De-esser reduction"><i style:width={grBar(deessGr)}></i><span class="num">{deessGr > 0.3 ? `−${deessGr.toFixed(1)} dB` : 'idle'}</span></div>
    </section>

    <section class="panel mod" class:off={!on('smart/enabled')} aria-label="Smart Volume">
      <div class="mod-head">
        <span class="mod-title"><Icon name="sliders" size={15} />Smart Volume</span>
        <Switch label="Smart Volume" checked={on('smart/enabled')} onchange={set('smart/enabled')} />
      </div>
      <div class="knobs">
        <Knob label="Target" min={-40} max={-10} step={0.5} reset={-20} size={64} color={meta.color} disabled={!on('smart/enabled')} value={g('smart/target_db', -20)} format={(v) => `${v.toFixed(0)} dB`} onchange={set('smart/target_db')} />
      </div>
      <p class="note num">Riding {db(smartGain)}</p>
    </section>

    <section class="panel mod wide" class:off={!on('comp/enabled')} aria-label="Compressor">
      <div class="mod-head">
        <span class="mod-title"><Icon name="sliders" size={15} />Compressor</span>
        <Switch label="Compressor" checked={on('comp/enabled')} onchange={set('comp/enabled')} />
      </div>
      <div class="knobs">
        <Knob label="Thresh" min={-60} max={0} step={0.5} reset={-24} size={52} color="var(--warn)" disabled={!on('comp/enabled')} value={g('comp/threshold_db', -24)} format={db} onchange={set('comp/threshold_db')} />
        <Knob label="Ratio" min={1} max={20} scale="log" step={0.1} reset={3} size={52} color="var(--warn)" disabled={!on('comp/enabled')} value={g('comp/ratio', 3)} format={(v) => `${v.toFixed(1)}:1`} onchange={set('comp/ratio')} />
        <Knob label="Attack" min={0.1} max={200} scale="log" step={0.1} reset={8} size={52} color="var(--warn)" disabled={!on('comp/enabled')} value={g('comp/attack_ms', 8)} format={ms} onchange={set('comp/attack_ms')} />
        <Knob label="Release" min={10} max={2000} scale="log" step={1} reset={140} size={52} color="var(--warn)" disabled={!on('comp/enabled')} value={g('comp/release_ms', 140)} format={ms} onchange={set('comp/release_ms')} />
        <Knob label="Makeup" min={0} max={24} step={0.5} reset={4} size={52} color="var(--warn)" disabled={!on('comp/enabled')} value={g('comp/makeup_db', 4)} format={db} onchange={set('comp/makeup_db')} />
      </div>
      <div class="gr" aria-label="Compressor reduction"><i style:width={grBar(compGr)}></i><span class="num">{compGr > 0.3 ? `−${compGr.toFixed(1)} dB` : 'idle'}</span></div>
    </section>

    <section class="panel mod" class:off={!on('limiter/enabled')} aria-label="Limiter">
      <div class="mod-head">
        <span class="mod-title"><Icon name="alert" size={15} />Limiter</span>
        <Switch label="Limiter" checked={on('limiter/enabled')} onchange={set('limiter/enabled')} />
      </div>
      <div class="knobs">
        <Knob label="Ceiling" min={-12} max={0} step={0.1} reset={-1} size={52} color="var(--bad)" disabled={!on('limiter/enabled')} value={g('limiter/ceiling_db', -1)} format={db} onchange={set('limiter/ceiling_db')} />
        <Knob label="Release" min={1} max={500} scale="log" step={1} reset={60} size={52} color="var(--bad)" disabled={!on('limiter/enabled')} value={g('limiter/release_ms', 60)} format={ms} onchange={set('limiter/release_ms')} />
      </div>
      <div class="gr" aria-label="Limiter reduction"><i style:width={grBar(limitGr)}></i><span class="num">{limitGr > 0.3 ? `−${limitGr.toFixed(1)} dB` : 'idle'}</span></div>
    </section>

    <section class="panel mod" aria-label="Monitoring">
      <div class="mod-head">
        <span class="mod-title"><Icon name="headphones" size={15} />Hear yourself</span>
        <Switch label="Hear yourself" checked={monitoring} onchange={(v) => engine.set(p('bus_mask'), (g('bus_mask') & ~1) | (v ? 1 : 0))} />
      </div>
      <p class="note">Your processed voice in your headphones. Use headphones: speakers feed back.</p>
    </section>
  </div>

  <MicCheck />
</div>

<style>
  .voice {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 1240px;
    margin: 0 auto;
    padding-bottom: 12px;
  }

  .hero {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    padding: 4px 2px;
  }

  .id {
    display: flex;
    align-items: center;
    gap: 14px;
    min-width: 0;
  }

  .badge {
    flex: none;
    width: 52px;
    height: 52px;
    border-radius: 16px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: radial-gradient(circle at 30% 20%, color-mix(in oklab, var(--c) 40%, transparent), color-mix(in oklab, var(--c) 10%, transparent));
    box-shadow: 0 0 0 1px color-mix(in oklab, var(--c) 45%, transparent), 0 10px 30px -10px var(--c);
  }

  h1 {
    font-size: 26px;
    font-weight: 750;
    letter-spacing: -0.02em;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .mute {
    display: flex;
    align-items: center;
    gap: 8px;
    height: 38px;
    padding: 0 16px;
    border-radius: 999px;
    font-weight: 650;
    border: 1px solid color-mix(in oklab, var(--ok) 50%, transparent);
    background: color-mix(in oklab, var(--ok) 14%, transparent);
    color: var(--text);
  }

  .mute.on {
    border-color: color-mix(in oklab, var(--bad) 60%, transparent);
    background: color-mix(in oklab, var(--bad) 18%, transparent);
    box-shadow: 0 0 24px -8px var(--bad);
  }

  .save {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px;
  }

  .rack {
    display: grid;
    grid-template-columns: repeat(4, minmax(0, 1fr));
    gap: 12px;
  }

  .mod {
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 16px 16px;
    transition: opacity 0.15s var(--ease);
  }

  .mod.wide {
    grid-column: span 2;
  }

  .mod.off .knobs,
  .mod.off .gr,
  .mod.off :global(.seg) {
    opacity: 0.45;
  }

  .mod-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
  }

  .mod-title {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 650;
    font-size: 13.5px;
    color: var(--text);
  }

  .mod-title :global(svg) {
    color: var(--c);
  }

  .knobs {
    display: flex;
    flex-wrap: wrap;
    justify-content: space-around;
    gap: 10px;
  }

  .note {
    font-size: 12px;
    color: var(--muted);
  }

  .led {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--bg-0);
    border: 1px solid var(--line-2);
  }

  .led.lit {
    background: var(--ok);
    border-color: transparent;
    box-shadow: 0 0 10px var(--ok);
  }

  .gr {
    position: relative;
    height: 18px;
    border-radius: 6px;
    background: var(--bg-0);
    overflow: hidden;
  }

  .gr i {
    position: absolute;
    inset: 0 auto 0 0;
    background: linear-gradient(90deg, color-mix(in oklab, var(--warn) 50%, transparent), var(--warn));
    transition: width 60ms linear;
  }

  .gr span {
    position: absolute;
    right: 8px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 11px;
    color: var(--text-2);
  }

  @media (max-width: 1180px) {
    .rack {
      grid-template-columns: repeat(2, minmax(0, 1fr));
    }
  }
</style>
