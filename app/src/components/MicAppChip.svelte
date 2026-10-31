<script lang="ts">
  import { appColor, appLabel } from '../lib/channels.ts';
  import { dnd } from '../lib/dnd.svelte.ts';
  import { engine, type MicApp } from '../lib/engine.svelte.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';

  interface Props {
    app: MicApp;
    /** Inside the Mic channel: smaller, with a remove button. */
    compact?: boolean;
  }
  let { app, compact = false }: Props = $props();

  const color = $derived(appColor(app.name));
  const label = $derived(appLabel(app.name));
  const level = $derived(Math.min(1, Math.sqrt(app.peak) * 1.15));
  const dragging = $derived(dnd.drag?.name === app.name);

  async function route(on: boolean) {
    try {
      await engine.routeMic(app.name, on);
      toast(on ? `${label} now hears your processed voice` : `${label} uses its own microphone again`, 'ok');
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  function drop(_name: string, target: string) {
    if (target === 'mic') void route(true);
    else if (target === 'none') void route(false);
    else toast(`${label} records sound: drop it on the Mic channel`, 'error');
  }
</script>

<div
  class="chip-mic"
  class:compact
  class:idle={!app.active}
  class:dragging
  style:--c={color}
  onpointerdown={(e) => dnd.begin(e, app.name, drop)}
  role="listitem"
  title={app.routed ? `${label} records Audiora Microphone` : `${label} records ${app.device_name ?? 'a microphone'}`}
>
  <span class="avatar" aria-hidden="true">{label.charAt(0)}<i><Icon name="mic" size={9} /></i></span>
  <span class="body">
    <span class="name">{label}</span>
    <span class="meter" aria-hidden="true"><i style:width="{level * 100}%"></i></span>
  </span>
  {#if compact}
    <button class="icon" aria-label="Give {label} its own microphone back" title="Use its own microphone again" onclick={() => route(false)}>
      <Icon name="x" size={12} />
    </button>
  {:else}
    <button class="icon" aria-label="Send {label} to the Mic channel" title="Give it your processed voice" onclick={() => route(true)}>
      <Icon name="link" size={13} />
    </button>
  {/if}
</div>

<style>
  .chip-mic {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 7px 7px 7px 8px;
    border-radius: 12px;
    background: var(--bg-2);
    border: 1px solid var(--line);
    cursor: grab;
    touch-action: none;
    transition: border-color 0.15s, opacity 0.15s;
  }

  .chip-mic:hover {
    border-color: color-mix(in oklab, var(--c) 45%, transparent);
  }

  .chip-mic.dragging {
    opacity: 0.35;
  }

  .chip-mic.idle {
    opacity: 0.6;
  }

  .avatar {
    position: relative;
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    font-weight: 750;
    font-size: 13px;
    color: #0b0e13;
    background: linear-gradient(135deg, var(--c), color-mix(in oklab, var(--c) 60%, #000));
  }

  /* a small microphone badge tells these apart from apps that play sound */
  .avatar i {
    position: absolute;
    right: -4px;
    bottom: -4px;
    width: 15px;
    height: 15px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #fff;
    background: #2ccbe0;
    box-shadow: 0 0 0 2px var(--bg-2);
  }

  .body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .name {
    font-weight: 600;
    font-size: 12.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .meter {
    height: 3px;
    border-radius: 2px;
    background: var(--bg-0);
    overflow: hidden;
  }

  .meter i {
    display: block;
    height: 100%;
    background: #2ccbe0;
    transition: width 0.12s linear;
  }

  .icon {
    flex: none;
    width: 24px;
    height: 24px;
    display: grid;
    place-items: center;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--muted);
  }

  .icon:hover {
    color: var(--text);
    background: var(--bg-3);
  }

  .compact {
    padding: 5px 5px 5px 6px;
    gap: 8px;
  }

  .compact .avatar {
    width: 22px;
    height: 22px;
    font-size: 11px;
  }
</style>
