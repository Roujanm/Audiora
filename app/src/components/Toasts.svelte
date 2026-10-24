<script lang="ts">
  import { fly } from 'svelte/transition';
  import { toasts } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';
</script>

<div class="stack" aria-live="polite" aria-atomic="false">
  {#each toasts.items as t (t.id)}
    <div class="toast {t.kind}" role={t.kind === 'error' ? 'alert' : 'status'} in:fly={{ y: 14, duration: 180 }} out:fly={{ y: 8, duration: 140 }}>
      <span class="ico"><Icon name={t.kind === 'error' ? 'alert' : 'check'} size={15} stroke={2.2} /></span>
      <span class="txt">{t.text}</span>
      <button class="x" aria-label="Dismiss" onclick={() => toasts.dismiss(t.id)}>×</button>
    </div>
  {/each}
</div>

<style>
  .stack {
    position: fixed;
    right: 22px;
    bottom: 54px;
    z-index: 100;
    display: flex;
    flex-direction: column;
    gap: 8px;
    align-items: flex-end;
    pointer-events: none;
  }

  .toast {
    pointer-events: auto;
    display: flex;
    align-items: center;
    gap: 10px;
    max-width: 420px;
    padding: 10px 10px 10px 12px;
    border-radius: var(--r-md);
    background: linear-gradient(180deg, var(--bg-4), var(--bg-3));
    box-shadow: var(--shadow-pop);
    font-weight: 550;
  }

  .ico {
    flex: none;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    border-radius: 50%;
    color: var(--on-accent);
    background: var(--accent);
  }

  .toast.ok .ico {
    background: var(--ok);
    color: #04170e;
  }

  .toast.error .ico {
    background: var(--bad);
    color: #230408;
  }

  .toast.error {
    box-shadow: var(--shadow-pop), 0 0 0 1px color-mix(in oklab, var(--bad) 45%, transparent);
  }

  .txt {
    min-width: 0;
    overflow-wrap: anywhere;
  }

  .x {
    flex: none;
    width: 24px;
    height: 24px;
    border: 0;
    border-radius: 6px;
    background: none;
    color: var(--muted);
    font-size: 17px;
    line-height: 1;
  }

  .x:hover {
    background: var(--bg-4);
    color: var(--text);
  }
</style>
