<script lang="ts">
  import { channelMeta } from '../lib/channels.ts';
  import { dnd } from '../lib/dnd.svelte.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import type { ChannelEntry } from '../lib/protocol.ts';
  import { shortName } from '../lib/theme.ts';
  import AppChip from './AppChip.svelte';
  import Fader from './Fader.svelte';
  import Icon from './Icon.svelte';
  import LedMeter from './LedMeter.svelte';
  import Pill from './Pill.svelte';
  import PresetPicker from './PresetPicker.svelte';

  interface Props {
    ch: ChannelEntry;
    onopen: () => void;
    onsetup: () => void;
  }
  let { ch, onopen, onsetup }: Props = $props();

  const meta = $derived(channelMeta(ch.key)!);
  const isMic = $derived(ch.kind === 'mic');
  const p = (leaf: string) => `strip/${ch.strip}/${leaf}`;
  const on = (leaf: string) => engine.get(p(leaf)) > 0.5;
  const apps = $derived(engine.appGroups.filter((a) => a.route === ch.key));
  const over = $derived(dnd.over === ch.key && dnd.drag !== null);
  const eqOn = $derived([0, 1, 2, 3, 4, 5].some((b) => Math.abs(engine.get(p(`eq/${b}/gain_db`))) >= 0.01));
  const gr = $derived(on('comp/enabled') ? engine.gauge(p('comp/gr_db')) : 0);
  const smartGain = $derived(on('smart/enabled') ? engine.gauge(p('smart/gain_db')) : 0);
  const monitoring = $derived((engine.get(p('bus_mask')) & 1) === 1);
  const tags = $derived([
    { id: 'EQ', on: eqOn },
    { id: 'BOOST', on: engine.get(p('boost_db')) > 0.05 },
    { id: 'SMART', on: on('smart/enabled') },
    ...(isMic ? [{ id: 'NR', on: on('denoise') }, { id: 'GATE', on: on('gate/enabled') }] : []),
    { id: 'COMP', on: on('comp/enabled') },
  ]);
  const sub = $derived(
    ch.online
      ? isMic
        ? shortName(ch.device_name ?? 'Microphone')
        : `${apps.length} ${apps.length === 1 ? 'app' : 'apps'}`
      : isMic
        ? 'No microphone'
        : 'No device',
  );
</script>

<section
  class="col panel"
  class:over
  class:offline={!ch.online}
  style:--c={meta.color}
  data-drop={isMic ? undefined : ch.key}
  aria-label="{meta.name} channel"
>
  <i class="topbar"></i>

  <header>
    <button class="badge" onclick={onopen} aria-label="Open the {meta.name} page" title="Open the {meta.name} page"><Icon name={meta.icon} size={16} /></button>
    <button class="meta" onclick={onopen} title="Open the {meta.name} page">
      <h3>{meta.name}</h3>
      <span class="sub">{sub}</span>
    </button>
  </header>

  <PresetPicker channel={ch.key} variant="chip" color={meta.color} />

  <div class="main">
    <Fader value={engine.get(p('gain_db'))} color={meta.color} label="{meta.name} level" onchange={(db) => engine.set(p('gain_db'), db)} />
    <LedMeter
      levels={[engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))]}
      holds={[engine.holdDb(p('peak_l')), engine.holdDb(p('peak_r'))]}
    />
  </div>

  <div class="pills">
    <Pill active={on('mute')} variant="mute" title="Mute {meta.name}" label="Mute {meta.name}" onclick={() => engine.toggle(p('mute'))}>MUTE</Pill>
    {#if isMic}
      <Pill
        active={monitoring}
        variant="ok"
        title={monitoring ? 'You hear your own microphone' : 'Hear your own microphone in the headphones'}
        label="Monitor microphone"
        onclick={() => engine.set(p('bus_mask'), engine.get(p('bus_mask')) ^ 1)}
      >MON</Pill>
    {:else}
      <Pill active={on('solo')} variant="solo" title="Solo {meta.name}" label="Solo {meta.name}" onclick={() => engine.toggle(p('solo'))}>SOLO</Pill>
    {/if}
  </div>

  <div class="tags" aria-label="Active processing">
    {#each tags as t (t.id)}
      <span class="tag" class:on={t.on}>{t.id}{#if t.id === 'COMP' && gr > 0.3}<b class="num"> −{gr.toFixed(0)}</b>{:else if t.id === 'SMART' && Math.abs(smartGain) > 0.5}<b class="num"> {smartGain > 0 ? '+' : '−'}{Math.abs(smartGain).toFixed(0)}</b>{/if}</span>
    {/each}
  </div>

  {#if isMic}
    <div class="drop mic">
      <span class="muted">{ch.online ? 'Your voice goes to the apps that listen to the virtual microphone.' : (ch.error ?? 'No microphone found.')}</span>
    </div>
  {:else}
    <div class="drop" role="list" aria-label="Apps in {meta.name}">
      {#each apps as app (app.name)}
        <AppChip {app} compact />
      {:else}
        {#if ch.online}
          <span class="hint"><Icon name="plus" size={14} /> Drop apps here</span>
        {:else}
          <button class="setup" onclick={onsetup}>
            <Icon name="alert" size={14} /> Pick a virtual device
          </button>
        {/if}
      {/each}
    </div>
  {/if}
</section>

<style>
  .col {
    flex: 1 1 0;
    min-width: 164px;
    max-width: 260px;
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px 14px 12px;
    min-height: 0;
    overflow: hidden;
    transition: border-color 0.15s, box-shadow 0.2s, background 0.2s;
  }

  .col.over {
    border-color: color-mix(in oklab, var(--c) 70%, transparent);
    box-shadow: var(--shadow-2), 0 0 0 1px color-mix(in oklab, var(--c) 40%, transparent), 0 0 34px -8px var(--c);
    background: linear-gradient(180deg, color-mix(in oklab, var(--c) 12%, var(--bg-2)), var(--bg-1));
  }

  .col.offline .main {
    opacity: 0.4;
    pointer-events: none;
  }

  .topbar {
    position: absolute;
    inset: 0 0 auto 0;
    height: 3px;
    background: var(--c);
    box-shadow: 0 0 18px 1px color-mix(in oklab, var(--c) 70%, transparent);
    border-radius: var(--r-lg) var(--r-lg) 0 0;
  }

  header {
    display: flex;
    gap: 9px;
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
    padding: 0;
    color: var(--c);
    background: color-mix(in oklab, var(--c) 14%, transparent);
    border: 1px solid color-mix(in oklab, var(--c) 30%, transparent);
    transition: transform 0.12s;
  }

  .badge:hover {
    transform: scale(1.06);
  }

  .meta {
    min-width: 0;
    flex: 1;
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    border: 0;
    background: none;
    padding: 0;
    text-align: left;
    color: var(--text);
  }

  h3 {
    font-size: 14px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  .sub {
    max-width: 100%;
    font-size: 10.5px;
    color: var(--faint);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .main {
    display: flex;
    gap: 8px;
    flex: 1;
    min-height: 140px;
    justify-content: center;
    transition: opacity 0.2s;
  }

  .pills {
    display: flex;
    gap: 5px;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    gap: 4px;
    min-height: 20px;
  }

  .tag {
    font: 700 8.5px var(--font);
    letter-spacing: 0.08em;
    padding: 2px 5px;
    border-radius: 4px;
    color: var(--faint);
    background: var(--bg-0);
    border: 1px solid var(--line);
  }

  .tag.on {
    color: var(--c);
    border-color: color-mix(in oklab, var(--c) 40%, transparent);
    background: color-mix(in oklab, var(--c) 12%, transparent);
  }

  .tag b {
    font-weight: 700;
    color: var(--warn);
  }

  .drop {
    flex: none;
    height: 112px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 6px;
    border-radius: var(--r-md);
    background: var(--bg-0);
    border: 1px dashed var(--line-2);
    box-shadow: inset 0 2px 6px rgba(0, 0, 0, 0.45);
    transition: border-color 0.15s, background 0.15s;
  }

  .col.over .drop {
    border-color: var(--c);
    background: color-mix(in oklab, var(--c) 8%, var(--bg-0));
  }

  .drop.mic {
    justify-content: center;
    text-align: center;
    font-size: 11px;
    border-style: solid;
  }

  .hint {
    margin: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--faint);
  }

  .setup {
    margin: auto;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 10px;
    border-radius: 8px;
    border: 1px solid color-mix(in oklab, var(--warn) 35%, transparent);
    background: color-mix(in oklab, var(--warn) 10%, transparent);
    color: var(--warn);
    font-size: 11.5px;
    font-weight: 600;
  }

  .setup:hover {
    background: color-mix(in oklab, var(--warn) 18%, transparent);
  }

  @media (max-height: 780px) {
    .col {
      gap: 8px;
      padding: 12px 12px 10px;
    }

    .main {
      min-height: 100px;
    }

    .drop {
      height: 84px;
    }
  }
</style>
