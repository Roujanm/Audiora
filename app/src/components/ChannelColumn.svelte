<script lang="ts">
  import { channelMeta } from '../lib/channels.ts';
  import { dnd } from '../lib/dnd.svelte.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import type { ChannelEntry } from '../lib/protocol.ts';
  import { shortName } from '../lib/theme.ts';
  import AppChip from './AppChip.svelte';
  import Icon from './Icon.svelte';
  import VolumeSlider from './VolumeSlider.svelte';

  interface Props {
    ch: ChannelEntry;
    onopen: () => void;
    onsetup: () => void;
  }
  let { ch, onopen, onsetup }: Props = $props();

  const meta = $derived(channelMeta(ch.key)!);
  const isMic = $derived(ch.kind === 'mic');
  const p = (leaf: string) => `strip/${ch.strip}/${leaf}`;
  const muted = $derived(engine.get(p('mute')) > 0.5);
  const apps = $derived(engine.appGroups.filter((a) => a.route === ch.key));
  const over = $derived(dnd.over === ch.key && dnd.drag !== null);
  const level = $derived(Math.max(engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))));
  const monitoring = $derived((engine.get(p('bus_mask')) & 1) === 1);
  const status = $derived(
    !ch.online
      ? isMic
        ? 'No microphone'
        : 'Not set up'
      : isMic
        ? shortName(ch.device_name ?? 'Microphone')
        : apps.length
          ? `${apps.length} ${apps.length === 1 ? 'app' : 'apps'}`
          : 'No apps yet',
  );
</script>

<section class="col panel" class:over class:offline={!ch.online} style:--c={meta.color} data-drop={isMic ? undefined : ch.key} aria-label="{meta.name} channel">
  <button class="head" onclick={onopen} title="Open {meta.name} settings">
    <span class="badge"><Icon name={meta.icon} size={17} /></span>
    <span class="txt">
      <strong>{meta.name}</strong>
      <small>{status}</small>
    </span>
    <span class="go"><Icon name="arrow" size={15} /></span>
  </button>

  <VolumeSlider
    db={engine.get(p('gain_db'))}
    {level}
    color={meta.color}
    {muted}
    disabled={!ch.online}
    label="{meta.name} volume"
    onchange={(db) => engine.set(p('gain_db'), db)}
  />

  <div class="actions">
    <button class="round" class:on={muted} aria-pressed={muted} title={muted ? 'Unmute' : 'Mute'} aria-label="Mute {meta.name}" onclick={() => engine.toggle(p('mute'))}>
      <Icon name={muted ? 'mute' : 'volume'} size={17} />
    </button>
    {#if isMic}
      <button
        class="round"
        class:lit={monitoring}
        aria-pressed={monitoring}
        title={monitoring ? 'Stop hearing yourself' : 'Hear yourself (headphones only)'}
        aria-label="Hear yourself"
        onclick={() => engine.set(p('bus_mask'), engine.get(p('bus_mask')) ^ 1)}
      >
        <Icon name="headphones" size={17} />
      </button>
    {/if}
  </div>

  {#if !isMic}
    <div class="apps" role="list" aria-label="Apps in {meta.name}">
      {#each apps as app (app.name)}
        <AppChip {app} compact />
      {:else}
        {#if ch.online}
          <span class="hint">Drop apps here</span>
        {:else}
          <button class="setup" onclick={onsetup}>Set up</button>
        {/if}
      {/each}
    </div>
  {/if}
</section>

<style>
  .col {
    flex: 1 1 0;
    min-width: 150px;
    max-width: 230px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 12px 12px 14px;
    min-height: 0;
    overflow: hidden;
    transition: border-color 0.15s, box-shadow 0.2s, background 0.2s;
  }

  .col::before {
    content: '';
    position: absolute;
    inset: 0 0 auto 0;
    height: 120px;
    border-radius: var(--r-lg) var(--r-lg) 0 0;
    background: radial-gradient(160px 90px at 50% 0%, color-mix(in oklab, var(--c) 14%, transparent), transparent 75%);
    pointer-events: none;
  }

  .col.over {
    border-color: color-mix(in oklab, var(--c) 70%, transparent);
    box-shadow: var(--shadow-2), 0 0 0 1px color-mix(in oklab, var(--c) 40%, transparent), 0 0 34px -8px var(--c);
  }

  .head {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px;
    margin: -4px -4px 0;
    border: 0;
    border-radius: 12px;
    background: none;
    color: var(--text);
    text-align: left;
    transition: background 0.15s;
  }

  .head:hover {
    background: color-mix(in oklab, var(--c) 10%, transparent);
  }

  .badge {
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: color-mix(in oklab, var(--c) 15%, transparent);
  }

  .txt {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  strong {
    font-size: 14.5px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }

  small {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .offline small {
    color: var(--warn);
  }

  .go {
    color: var(--faint);
    transition: color 0.15s, transform 0.15s;
  }

  .head:hover .go {
    color: var(--c);
    transform: translateX(2px);
  }

  .actions {
    display: flex;
    justify-content: center;
    gap: 10px;
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
    transition: all 0.15s;
  }

  .round:hover {
    color: var(--text);
    border-color: var(--line-3);
  }

  .round.on {
    color: #fff;
    background: var(--bad);
    border-color: transparent;
    box-shadow: 0 0 16px -4px var(--bad);
  }

  .round.lit {
    color: var(--c);
    border-color: color-mix(in oklab, var(--c) 60%, transparent);
    background: color-mix(in oklab, var(--c) 14%, transparent);
  }

  .apps {
    flex: none;
    height: 104px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding: 6px;
    border-radius: 12px;
    border: 1px dashed var(--line-2);
    transition: border-color 0.15s, background 0.15s;
  }

  .col.over .apps {
    border-color: var(--c);
    background: color-mix(in oklab, var(--c) 8%, transparent);
  }

  .hint {
    margin: auto;
    font-size: 11.5px;
    color: var(--faint);
  }

  .setup {
    margin: auto;
    height: 30px;
    padding: 0 16px;
    border-radius: 999px;
    border: 1px solid color-mix(in oklab, var(--warn) 40%, transparent);
    background: color-mix(in oklab, var(--warn) 10%, transparent);
    color: var(--warn);
    font-size: 12px;
    font-weight: 600;
  }

  .setup:hover {
    background: color-mix(in oklab, var(--warn) 18%, transparent);
  }

  @media (max-width: 1240px) {
    .col {
      min-width: 128px;
      padding: 10px 10px 12px;
    }

    .badge {
      width: 32px;
      height: 32px;
    }
  }

  @media (max-height: 780px) {
    .col {
      gap: 10px;
    }

    .apps {
      height: 80px;
    }

    .round {
      width: 34px;
      height: 34px;
    }
  }
</style>
