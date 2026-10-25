<script lang="ts">
  import { dnd } from '../lib/dnd.svelte.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import AppChip from './AppChip.svelte';
  import Icon from './Icon.svelte';

  const unrouted = $derived(engine.appGroups.filter((a) => a.route === null));
  const over = $derived(dnd.over === 'none' && dnd.drag !== null);
</script>

<section class="dock panel" class:over data-drop="none" aria-label="Detected applications">
  <header>
    <div>
      <h2>Apps</h2>
      <p class="muted">Drag one onto a channel</p>
    </div>
    <span class="count num">{unrouted.length}</span>
  </header>

  <div class="list" role="list">
    {#each unrouted as app (app.name)}
      <AppChip {app} />
    {:else}
      <div class="empty">
        <span class="ico"><Icon name="wave" size={22} /></span>
        <p><strong>Waiting for sound</strong></p>
        <p class="muted">Programs that play audio show up here by themselves.</p>
      </div>
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

  .count {
    min-width: 20px;
    height: 20px;
    padding: 0 6px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    font-size: 11px;
    color: var(--text-2);
    background: var(--bg-0);
    border: 1px solid var(--line);
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

  .empty {
    margin: auto 0;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 12px 6px;
    font-size: 12px;
  }

  .empty .ico {
    width: 42px;
    height: 42px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: var(--accent-hi);
    background: var(--accent-soft);
    margin-bottom: 6px;
  }

  h2 {
    font-size: 14.5px;
    font-weight: 700;
  }

  header p {
    font-size: 11px;
  }
</style>
