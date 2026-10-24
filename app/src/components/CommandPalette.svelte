<script lang="ts">
  import { fade, fly } from 'svelte/transition';
  import Icon, { type IconName } from './Icon.svelte';

  export interface Command {
    id: string;
    title: string;
    group: string;
    icon: IconName;
    hint?: string;
    run: () => void;
  }

  interface Props {
    open: boolean;
    commands: Command[];
    onclose: () => void;
  }
  let { open, commands, onclose }: Props = $props();

  let query = $state('');
  let active = $state(0);
  let input = $state<HTMLInputElement>();
  let list = $state<HTMLDivElement>();

  const results = $derived.by(() => {
    const q = query.trim().toLowerCase();
    if (!q) return commands;
    return commands
      .map((c) => ({ c, score: `${c.title} ${c.group}`.toLowerCase().indexOf(q) }))
      .filter((x) => x.score >= 0)
      .sort((a, b) => a.score - b.score)
      .map((x) => x.c);
  });

  $effect(() => {
    if (open) {
      query = '';
      active = 0;
      queueMicrotask(() => input?.focus());
    }
  });

  $effect(() => {
    void results.length;
    if (active >= results.length) active = Math.max(results.length - 1, 0);
  });

  $effect(() => {
    list?.querySelector('[aria-selected="true"]')?.scrollIntoView({ block: 'nearest' });
  });

  function run(c: Command | undefined) {
    if (!c) return;
    onclose();
    c.run();
  }

  function onkeydown(e: KeyboardEvent) {
    if (e.key === 'ArrowDown') {
      e.preventDefault();
      active = (active + 1) % Math.max(results.length, 1);
    } else if (e.key === 'ArrowUp') {
      e.preventDefault();
      active = (active - 1 + results.length) % Math.max(results.length, 1);
    } else if (e.key === 'Enter') {
      e.preventDefault();
      run(results[active]);
    } else if (e.key === 'Escape') {
      e.preventDefault();
      onclose();
    }
  }
</script>

{#if open}
  <!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
  <div class="scrim" transition:fade={{ duration: 120 }} onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
    <div class="sheet" role="dialog" aria-modal="true" aria-label="Command palette" transition:fly={{ y: -10, duration: 160 }}>
      <div class="field">
        <Icon name="bolt" size={17} />
        <input
          bind:this={input}
          type="text"
          placeholder="Type a command or search presets…"
          aria-label="Search commands"
          role="combobox"
          aria-expanded="true"
          aria-controls="cmd-list"
          bind:value={query}
          oninput={() => (active = 0)}
          {onkeydown}
        />
        <kbd>Esc</kbd>
      </div>
      <div class="list" id="cmd-list" role="listbox" bind:this={list}>
        {#each results as c, i (c.id)}
          {#if i === 0 || results[i - 1].group !== c.group}<p class="group eyebrow">{c.group}</p>{/if}
          <button
            class="row"
            class:active={i === active}
            role="option"
            aria-selected={i === active}
            onpointermove={() => (active = i)}
            onclick={() => run(c)}
          >
            <span class="ico"><Icon name={c.icon} size={16} /></span>
            <span class="t">{c.title}</span>
            {#if c.hint}<kbd>{c.hint}</kbd>{/if}
          </button>
        {:else}
          <p class="none muted">No command matches “{query}”.</p>
        {/each}
      </div>
      <footer><span><kbd>↑</kbd> <kbd>↓</kbd> navigate</span><span><kbd>Enter</kbd> run</span><span><kbd>Ctrl</kbd> <kbd>K</kbd> toggle</span></footer>
    </div>
  </div>
{/if}

<style>
  .scrim {
    position: fixed;
    inset: 0;
    z-index: 90;
    display: flex;
    justify-content: center;
    align-items: flex-start;
    padding-top: 12vh;
    background: rgba(3, 5, 8, 0.62);
    backdrop-filter: blur(5px);
  }

  .sheet {
    width: min(560px, calc(100vw - 40px));
    border-radius: var(--r-lg);
    background: linear-gradient(180deg, var(--bg-3), var(--bg-2));
    box-shadow: var(--shadow-pop);
    overflow: hidden;
  }

  .field {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 16px;
    height: 54px;
    border-bottom: 1px solid var(--line);
    color: var(--accent-hi);
  }

  .field input {
    all: unset;
    flex: 1;
    height: 100%;
    color: var(--text);
    font-size: 15px;
    user-select: text;
  }

  .field input::placeholder {
    color: var(--faint);
  }

  .list {
    max-height: min(52vh, 420px);
    overflow: auto;
    padding: 6px;
  }

  .group {
    padding: 10px 10px 4px;
    font-size: 10px;
  }

  .row {
    all: unset;
    box-sizing: border-box;
    width: 100%;
    display: flex;
    align-items: center;
    gap: 12px;
    height: 38px;
    padding: 0 10px;
    border-radius: 9px;
    cursor: pointer;
    color: var(--text-2);
  }

  .row.active {
    background: var(--accent-soft);
    color: var(--text);
  }

  .ico {
    display: grid;
    place-items: center;
    width: 26px;
    height: 26px;
    border-radius: 7px;
    background: var(--bg-0);
    color: var(--muted);
  }

  .row.active .ico {
    color: var(--accent-hi);
  }

  .t {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    font-weight: 550;
  }

  kbd {
    font: 600 10.5px var(--mono);
    padding: 2px 6px;
    border-radius: 5px;
    background: var(--bg-0);
    border: 1px solid var(--line-2);
    color: var(--muted);
  }

  .none {
    padding: 22px 14px;
    text-align: center;
  }

  footer {
    display: flex;
    gap: 18px;
    padding: 9px 16px;
    border-top: 1px solid var(--line);
    font-size: 11px;
    color: var(--faint);
  }
</style>
