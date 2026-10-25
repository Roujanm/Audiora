<script lang="ts">
  import { desktop } from '../lib/desktop.svelte.ts';
  import Icon from './Icon.svelte';

  const busy = $derived(desktop.phase === 'downloading' || desktop.phase === 'installing');
</script>

{#if desktop.update && (!desktop.dismissed || busy)}
  <aside class="upd panel" role="status" aria-live="polite">
    <span class="ico"><Icon name="bolt" size={18} /></span>
    <div class="txt">
      <strong>Audiora {desktop.update.version} is available</strong>
      {#if busy}
        <p class="muted">{desktop.phase === 'installing' ? 'Installing… Audiora restarts by itself.' : `Downloading… ${Math.round(desktop.progress * 100)}%`}</p>
        <div class="bar"><i style:width="{desktop.phase === 'installing' ? 100 : desktop.progress * 100}%"></i></div>
      {:else}
        <p class="muted">{desktop.update.notes ? desktop.update.notes.split('\n')[0].slice(0, 120) : `You have ${desktop.version}.`}</p>
      {/if}
    </div>
    {#if !busy}
      <div class="act">
        <button class="btn sm primary" onclick={() => void desktop.install()}>Update now</button>
        <button class="btn sm ghost" onclick={() => (desktop.dismissed = true)}>Later</button>
      </div>
    {/if}
  </aside>
{/if}

<style>
  .upd {
    position: fixed;
    right: 20px;
    bottom: 54px;
    z-index: 120;
    width: 380px;
    display: flex;
    align-items: flex-start;
    gap: 12px;
    padding: 14px 16px;
    box-shadow: var(--shadow-pop);
    animation: up 0.3s var(--ease);
  }

  @keyframes up {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
  }

  .ico {
    flex: none;
    width: 36px;
    height: 36px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    color: var(--accent-hi);
    background: var(--accent-soft);
  }

  .txt {
    flex: 1;
    min-width: 0;
  }

  strong {
    font-size: 13.5px;
  }

  p {
    font-size: 12px;
    margin-top: 2px;
  }

  .bar {
    margin-top: 8px;
    height: 4px;
    border-radius: 2px;
    background: var(--bg-0);
    overflow: hidden;
  }

  .bar i {
    display: block;
    height: 100%;
    background: var(--accent);
    transition: width 0.2s;
  }

  .act {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
</style>
