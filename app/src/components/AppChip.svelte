<script lang="ts">
  import { appColor, appLabel, CHANNELS, channelMeta } from '../lib/channels.ts';
  import { dnd } from '../lib/dnd.svelte.ts';
  import { engine, type AppGroup } from '../lib/engine.svelte.ts';
  import type { ChannelKey } from '../lib/protocol.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';
  import Popover from './Popover.svelte';

  interface Props {
    app: AppGroup;
    /** Inside a channel column: smaller, with a remove button. */
    compact?: boolean;
  }
  let { app, compact = false }: Props = $props();

  const color = $derived(appColor(app.name));
  const label = $derived(appLabel(app.name));
  const idle = $derived(app.sessions.length === 0);
  const level = $derived(Math.min(1, Math.sqrt(app.peak) * 1.15));
  const waiting = $derived(app.route !== null && !idle && app.playingIn !== app.route);
  const dragging = $derived(dnd.drag?.name === app.name);

  async function assign(channel: ChannelKey | null) {
    try {
      await engine.routeApp(app.name, channel);
      toast(`${label} → ${channel ? channelMeta(channel)?.name : 'not routed'}`, 'ok');
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  function drop(name: string, target: string) {
    void engine.routeApp(name, target === 'none' ? null : (target as ChannelKey)).then(
      () => toast(`${appLabel(name)} → ${target === 'none' ? 'not routed' : channelMeta(target)?.name}`, 'ok'),
      (e) => toast(String(e instanceof Error ? e.message : e), 'error'),
    );
  }
</script>

<div
  class="chip-app"
  class:compact
  class:idle
  class:dragging
  style:--c={color}
  onpointerdown={(e) => dnd.begin(e, app.name, drop)}
  role="listitem"
  title={idle ? `${label} is not running` : waiting ? `${label}: restart its audio so Windows moves it to the channel` : label}
>
  <span class="avatar" aria-hidden="true">{label.charAt(0)}</span>
  <span class="body">
    <span class="name">{label}</span>
    <span class="meter" aria-hidden="true"><i style:width="{level * 100}%"></i></span>
  </span>

  {#if compact}
    <button class="icon" aria-label="Remove {label} from this channel" title="Remove from this channel" onclick={() => assign(null)}>
      <Icon name="x" size={12} />
    </button>
  {:else}
    <Popover width={170}>
      {#snippet trigger({ open, toggle })}
        <button class="icon menu" class:open aria-label="Send {label} to a channel" title="Send to a channel" onclick={toggle}>
          <Icon name="link" size={13} />
        </button>
      {/snippet}
      {#snippet children({ close })}
        <ul class="menu-list" role="menu">
          {#each CHANNELS.filter((c) => c.key !== 'mic') as c (c.key)}
            <li>
              <button
                role="menuitem"
                onclick={() => {
                  close();
                  void assign(c.key);
                }}
              >
                <i style:background={c.color}></i>{c.name}
              </button>
            </li>
          {/each}
        </ul>
      {/snippet}
    </Popover>
  {/if}
  {#if waiting}<i class="wait" aria-hidden="true"></i>{/if}
</div>

<style>
  .chip-app {
    position: relative;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 8px 8px 9px;
    border-radius: var(--r-md);
    background: linear-gradient(180deg, var(--bg-3), var(--bg-2));
    border: 1px solid var(--line-2);
    box-shadow: var(--shadow-1);
    cursor: grab;
    touch-action: none;
    transition: border-color 0.15s, opacity 0.15s, transform 0.12s var(--ease);
  }

  .chip-app:hover {
    border-color: color-mix(in oklab, var(--c) 45%, transparent);
  }

  .chip-app.dragging {
    opacity: 0.35;
  }

  .chip-app.idle {
    opacity: 0.55;
  }

  .avatar {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    font-weight: 750;
    font-size: 13px;
    color: #0b0e13;
    background: linear-gradient(135deg, var(--c), color-mix(in oklab, var(--c) 60%, #000));
    box-shadow: 0 4px 12px -4px var(--c);
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
    border-radius: 2px;
    background: var(--c);
    transition: width 0.12s linear;
  }

  .icon {
    flex: none;
    width: 24px;
    height: 24px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    border: 1px solid transparent;
    background: none;
    color: var(--faint);
    padding: 0;
  }

  .icon:hover,
  .icon.open {
    color: var(--text);
    background: var(--bg-4);
  }

  .wait {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--warn);
    box-shadow: 0 0 8px var(--warn);
  }

  .compact {
    padding: 5px 5px 5px 7px;
    gap: 8px;
  }

  .compact .avatar {
    width: 22px;
    height: 22px;
    border-radius: 7px;
    font-size: 11px;
  }

  .compact .name {
    font-size: 11.5px;
  }

  .menu-list {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .menu-list button {
    width: 100%;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 32px;
    padding: 0 10px;
    border: 0;
    border-radius: 8px;
    background: none;
    color: var(--text-2);
    font-weight: 550;
    text-align: left;
  }

  .menu-list button:hover {
    background: var(--bg-4);
    color: var(--text);
  }

  .menu-list i {
    width: 9px;
    height: 9px;
    border-radius: 50%;
  }
</style>
