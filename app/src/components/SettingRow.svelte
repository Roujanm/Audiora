<script lang="ts">
  import type { Snippet } from 'svelte';
  import Icon, { type IconName } from './Icon.svelte';
  import Switch from './Switch.svelte';

  interface Props {
    title: string;
    description?: string;
    icon: IconName;
    /** With `ontoggle`, the row has an on/off switch and its control dims while off. */
    on?: boolean;
    ontoggle?: (on: boolean) => void;
    /** Short live value shown next to the title (e.g. "−4 dB"). */
    value?: string;
    disabled?: boolean;
    children?: Snippet;
  }
  let { title, description, icon, on, ontoggle, value, disabled = false, children }: Props = $props();
  const switchable = $derived(on !== undefined && ontoggle !== undefined);
</script>

<div class="row" class:off={switchable && !on} class:disabled>
  <span class="ico"><Icon name={icon} size={17} /></span>
  <div class="txt">
    <div class="t">{title}{#if value}<span class="v num">{value}</span>{/if}</div>
    {#if description}<p>{description}</p>{/if}
  </div>
  {#if children}<div class="ctl">{@render children()}</div>{:else}<span class="grow"></span>{/if}
  {#if switchable}<Switch checked={!!on} label={title} {disabled} onchange={(v) => ontoggle?.(v)} />{/if}
</div>

<style>
  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 14px 18px;
    min-height: 68px;
  }

  .row + :global(.row) {
    border-top: 1px solid var(--line);
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
    transition: all 0.2s;
  }

  .off .ico {
    color: var(--faint);
    background: var(--bg-3);
  }

  .txt {
    flex: 0 1 300px;
    min-width: 0;
  }

  .t {
    display: flex;
    align-items: baseline;
    gap: 10px;
    font-size: 13.5px;
    font-weight: 650;
  }

  .v {
    font-size: 12px;
    font-weight: 500;
    color: var(--accent-hi);
  }

  p {
    margin-top: 2px;
    font-size: 12px;
    color: var(--muted);
  }

  .ctl {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    justify-content: flex-end;
    gap: 12px;
    transition: opacity 0.2s;
  }

  .off .ctl {
    opacity: 0.4;
    pointer-events: none;
  }

  .grow {
    flex: 1;
  }

  .disabled {
    opacity: 0.5;
  }
</style>
