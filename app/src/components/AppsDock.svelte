<script lang="ts">
  import { dnd } from '../lib/dnd.svelte.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import AppChip from './AppChip.svelte';
  import Icon from './Icon.svelte';
  import MicAppChip from './MicAppChip.svelte';

  const unrouted = $derived(engine.appGroups.filter((a) => a.route === null));
  const mics = $derived(engine.micApps.filter((a) => !a.routed));
  const over = $derived(dnd.over === 'none' && dnd.drag !== null);
</script>

<section class="dock panel" class:over data-drop="none" aria-label="Detected applications">
  <header>
    <div>
      <h2>Apps</h2>
      <p class="muted">Drag one onto a channel</p>
    </div>
  </header>

  <div class="list" role="list">
    <p class="group"><Icon name="volume" size={12} /> Playing sound <span class="count num">{unrouted.length}</span></p>
    {#each unrouted as app (app.name)}
      <AppChip {app} />
    {:else}
      <p class="empty muted">Programs that play audio show up here by themselves.</p>
    {/each}

    <p class="group mic"><Icon name="mic" size={12} /> Using a microphone <span class="count num">{mics.length}</span></p>
    {#each mics as app (app.name)}
      <MicAppChip {app} />
    {:else}
      <p class="empty muted">
        {engine.hasAudioraMic ? 'HiNekst, Discord and games appear here while they use a microphone: drop them on Mic.' : 'Needs Audiora Microphone (installed with the driver).'}
      </p>
    {/each}
  </div>
</section>

<style>
  .dock {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 14px;
    min-height: 0;
    flex: 1;
    transition: border-color 0.15s, background 0.15s;
  }

  .dock.over {
    border-color: color-mix(in oklab, var(--accent) 55%, transparent);
    background: linear-gradient(180deg, color-mix(in oklab, var(--accent) 10%, var(--bg-2)), var(--bg-1));
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
  }

  .list {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 7px;
    padding-right: 3px;
    margin-right: -3px;
  }

  .group {
    display: flex;
    align-items: center;
    gap: 6px;
    margin: 4px 2px 0;
    font-size: 10.5px;
    font-weight: 700;
    letter-spacing: 0.08em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .group.mic {
    margin-top: 10px;
  }

  .count {
    margin-left: auto;
    min-width: 18px;
    height: 18px;
    padding: 0 5px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    font-size: 10.5px;
    color: var(--text-2);
    background: var(--bg-0);
    border: 1px solid var(--line);
    letter-spacing: 0;
  }

  .empty {
    font-size: 11.5px;
    padding: 2px 4px 4px;
  }

  h2 {
    font-size: 14.5px;
    font-weight: 700;
  }

  header p {
    font-size: 11px;
  }
</style>
