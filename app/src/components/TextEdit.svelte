<script lang="ts">
  interface Props {
    value: string;
    /** Shown in place of an empty name. */
    placeholder?: string;
    label: string;
    onchange: (value: string) => void;
  }
  let { value, placeholder = '', label, onchange }: Props = $props();

  let editing = $state(false);
  let draft = $state('');
  let input = $state<HTMLInputElement>();

  function start() {
    draft = value;
    editing = true;
    queueMicrotask(() => {
      input?.focus();
      input?.select();
    });
  }

  function commit() {
    if (!editing) return;
    editing = false;
    if (draft.trim() !== value) onchange(draft.trim());
  }
</script>

{#if editing}
  <input
    bind:this={input}
    class="rename"
    type="text"
    maxlength="40"
    aria-label="Rename {label}"
    {placeholder}
    bind:value={draft}
    onkeydown={(e) => {
      if (e.key === 'Enter') commit();
      else if (e.key === 'Escape') editing = false;
      e.stopPropagation();
    }}
    onblur={commit}
  />
{:else}
  <button class="name" type="button" title="Double-click to rename" ondblclick={start} onkeydown={(e) => e.key === 'F2' && start()}>
    {value || placeholder}
  </button>
{/if}

<style>
  .name {
    all: unset;
    box-sizing: border-box;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
    cursor: default;
    width: 100%;
    font: inherit;
    line-height: inherit;
    border-radius: 4px;
  }

  .name:hover {
    background: var(--bg-3);
  }

  .name:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .rename {
    width: 100%;
    min-width: 0;
    height: 24px;
    padding: 0 6px;
    font: inherit;
    font-size: 12.5px;
    font-weight: 600;
    border-radius: 6px;
    border: 1px solid var(--accent);
    background: var(--bg-0);
    color: var(--text);
    outline: none;
    box-shadow: 0 0 0 3px var(--accent-soft);
    user-select: text;
  }
</style>
