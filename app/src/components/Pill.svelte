<script lang="ts">
  import type { Snippet } from 'svelte';

  interface Props {
    active: boolean;
    variant?: 'accent' | 'mute' | 'solo' | 'ok' | 'bad';
    title?: string;
    label?: string;
    disabled?: boolean;
    onclick: () => void;
    children: Snippet;
  }
  let { active, variant = 'accent', title, label, disabled = false, onclick, children }: Props = $props();
</script>

<button
  class="pill {variant}"
  class:on={active}
  type="button"
  {title}
  {disabled}
  aria-pressed={active}
  aria-label={label}
  {onclick}
>
  {@render children()}
</button>

<style>
  .pill {
    flex: 1;
    min-width: 0;
    height: 26px;
    padding: 0 6px;
    border-radius: 6px;
    border: 1px solid var(--line-2);
    background: linear-gradient(180deg, var(--bg-3), var(--bg-2));
    color: var(--muted);
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.04em;
    box-shadow: var(--shadow-1);
    transition: color 0.12s, background 0.12s, border-color 0.12s, box-shadow 0.12s;
  }

  .pill:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--line-3);
  }

  .pill:disabled {
    opacity: 0.35;
    cursor: default;
  }

  .pill.on {
    color: var(--k-text);
    background: linear-gradient(180deg, color-mix(in oklab, var(--k) 100%, white 14%), var(--k));
    border-color: transparent;
    box-shadow: 0 0 14px -2px color-mix(in oklab, var(--k) 70%, transparent), 0 1px 0 rgba(255, 255, 255, 0.25) inset;
  }

  .accent {
    --k: var(--accent);
    --k-text: var(--on-accent);
  }
  .mute {
    --k: var(--bad);
    --k-text: #230408;
  }
  .solo {
    --k: var(--warn);
    --k-text: #231500;
  }
  .ok {
    --k: var(--ok);
    --k-text: #04170e;
  }
  .bad {
    --k: var(--rec);
    --k-text: #230409;
  }
</style>
