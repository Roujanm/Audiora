<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    /** Renders the trigger; call `toggle` to open or close. */
    trigger: Snippet<[{ open: boolean; toggle: () => void }]>;
    children: Snippet<[{ close: () => void }]>;
    align?: 'start' | 'end';
    width?: number;
  }
  let { trigger, children, align = 'end', width = 280 }: Props = $props();

  let open = $state(false);
  let root: HTMLDivElement;

  const toggle = () => (open = !open);
  const close = () => (open = false);

  $effect(() => {
    if (!open) return;
    const down = (e: PointerEvent) => {
      if (!root.contains(e.target as Node)) close();
    };
    const key = (e: KeyboardEvent) => e.key === 'Escape' && close();
    window.addEventListener('pointerdown', down, true);
    window.addEventListener('keydown', key);
    return () => {
      window.removeEventListener('pointerdown', down, true);
      window.removeEventListener('keydown', key);
    };
  });
</script>

<div class="pop" bind:this={root}>
  {@render trigger({ open, toggle })}
  {#if open}
    <div class="sheet {align}" style:width="{width}px" role="dialog">
      {@render children({ close })}
    </div>
  {/if}
</div>

<style>
  .pop {
    position: relative;
  }

  .sheet {
    position: absolute;
    top: calc(100% + 8px);
    z-index: 50;
    padding: 8px;
    border-radius: var(--r-md);
    background: linear-gradient(180deg, var(--bg-3), var(--bg-2));
    box-shadow: var(--shadow-pop);
    animation: in 0.14s var(--ease);
  }

  .sheet.end {
    right: 0;
  }

  .sheet.start {
    left: 0;
  }

  @keyframes in {
    from {
      opacity: 0;
      transform: translateY(-4px) scale(0.98);
    }
  }
</style>
