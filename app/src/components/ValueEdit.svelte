<script lang="ts">
  import { parseNumber } from '../lib/parse.ts';

  interface Props {
    /** What is shown while not editing. */
    text: string;
    label: string;
    /** Turns typed text into a value, or null to reject it. Defaults to plain numbers. */
    parse?: (text: string) => number | null;
    /** Text placed in the box when editing starts (defaults to `text`). */
    editText?: string;
    disabled?: boolean;
    onchange: (value: number) => void;
  }
  let { text, label, parse = parseNumber, editText, disabled = false, onchange }: Props = $props();

  let editing = $state(false);
  let draft = $state('');
  let bad = $state(false);
  let input = $state<HTMLInputElement>();

  function start() {
    if (disabled) return;
    draft = (editText ?? text).replace(/[^\d.+−\-,kK:LRCcl]/g, '').trim();
    bad = false;
    editing = true;
    queueMicrotask(() => {
      input?.focus();
      input?.select();
    });
  }

  function commit() {
    if (!editing) return;
    const v = parse(draft);
    if (v === null) {
      bad = true;
      return;
    }
    editing = false;
    onchange(v);
  }

  function cancel() {
    editing = false;
    bad = false;
  }
</script>

{#if editing}
  <input
    bind:this={input}
    class="edit num"
    class:bad
    type="text"
    inputmode="decimal"
    aria-label="{label} value"
    bind:value={draft}
    oninput={() => (bad = false)}
    onkeydown={(e) => {
      if (e.key === 'Enter') commit();
      else if (e.key === 'Escape') cancel();
      e.stopPropagation();
    }}
    onblur={() => (bad ? cancel() : commit())}
  />
{:else}
  <button class="val num" type="button" {disabled} title="Click to type a value" aria-label="{label}: {text}. Click to type a value" onclick={start}>
    {text}
  </button>
{/if}

<style>
  .val {
    all: unset;
    box-sizing: border-box;
    cursor: text;
    border-radius: 4px;
    padding: 0 3px;
    min-width: 3ch;
    text-align: center;
  }

  .val:hover:not(:disabled) {
    background: var(--bg-4);
    color: var(--text);
  }

  .val:focus-visible {
    outline: 2px solid var(--accent);
    outline-offset: 1px;
  }

  .edit {
    width: 8ch;
    height: 22px;
    padding: 0 4px;
    text-align: center;
    font-size: inherit;
    border-radius: 5px;
    border: 1px solid var(--accent);
    background: var(--bg-0);
    color: var(--text);
    outline: none;
    box-shadow: 0 0 0 3px var(--accent-soft);
  }

  .edit.bad {
    border-color: var(--bad);
    box-shadow: 0 0 0 3px color-mix(in oklab, var(--bad) 22%, transparent);
  }
</style>
