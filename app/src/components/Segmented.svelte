<script lang="ts">
  interface Option {
    id: string;
    label: string;
  }
  interface Props {
    options: Option[];
    value: string;
    label: string;
    size?: 'sm' | 'md';
    onchange: (id: string) => void;
  }
  let { options, value, label, size = 'md', onchange }: Props = $props();
</script>

<div class="seg {size}" role="group" aria-label={label}>
  {#each options as o (o.id)}
    <button type="button" class:active={o.id === value} aria-pressed={o.id === value} onclick={() => onchange(o.id)}>
      {o.label}
    </button>
  {/each}
</div>

<style>
  .seg {
    display: inline-flex;
    padding: 3px;
    gap: 2px;
    border-radius: var(--r-md);
    background: var(--bg-0);
    border: 1px solid var(--line);
    box-shadow: inset 0 1px 3px rgba(0, 0, 0, 0.6);
  }

  button {
    border: 0;
    background: none;
    color: var(--muted);
    font-weight: 600;
    border-radius: 8px;
    padding: 0 14px;
    height: 28px;
    transition: color 0.12s, background 0.12s;
  }

  .sm button {
    height: 24px;
    padding: 0 10px;
    font-size: 12px;
  }

  button:hover {
    color: var(--text);
  }

  button.active {
    color: var(--text);
    background: linear-gradient(180deg, var(--bg-4), var(--bg-3));
    box-shadow: var(--shadow-1), 0 0 0 1px var(--line-2);
  }
</style>
