<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import Switch from './Switch.svelte';

  interface Props {
    title: string;
    icon?: IconName;
    /** Accent colour of the icon badge and active state. */
    color?: string;
    /** When given, the header shows a switch and the body dims while off. */
    enabled?: boolean;
    onenabled?: (on: boolean) => void;
    description?: string;
    /** Small status text shown at the right of the header (e.g. a live value). */
    status?: string;
    children: Snippet;
    actions?: Snippet;
  }
  let { title, icon, color = 'var(--accent)', enabled, onenabled, description, status, children, actions }: Props = $props();
  const switchable = $derived(enabled !== undefined && onenabled !== undefined);
</script>

<section class="card panel" class:off={switchable && !enabled} style:--c={color}>
  <header>
    {#if icon}<span class="ico"><Icon name={icon} size={16} /></span>{/if}
    <div class="t">
      <h2>{title}</h2>
      {#if description}<p class="muted">{description}</p>{/if}
    </div>
    {#if status}<span class="status num">{status}</span>{/if}
    {#if actions}{@render actions()}{/if}
    {#if switchable}<Switch checked={!!enabled} label={title} onchange={(v) => onenabled?.(v)} />{/if}
  </header>
  <div class="body">{@render children()}</div>
</section>

<style>
  .card {
    padding: 16px 18px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-width: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .ico {
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: color-mix(in oklab, var(--c) 14%, transparent);
    border: 1px solid color-mix(in oklab, var(--c) 28%, transparent);
  }

  .t {
    flex: 1;
    min-width: 0;
  }

  h2 {
    font-size: 14px;
    font-weight: 650;
    letter-spacing: -0.005em;
  }

  .t p {
    font-size: 12px;
    margin-top: 1px;
  }

  .status {
    font-size: 12px;
    color: var(--c);
  }

  .body {
    transition: opacity 0.2s;
  }

  .off .body {
    opacity: 0.45;
  }

  .off .ico {
    color: var(--faint);
    background: var(--bg-3);
    border-color: var(--line);
  }
</style>
