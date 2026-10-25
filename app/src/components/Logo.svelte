<script lang="ts">
  import { GLYPH } from '../lib/glyph.ts';
  import { MARK, WORDMARK } from '../lib/logo.ts';

  interface Props {
    /** `glyph`: the bold small-size version of the mark (menus, icons). */
    variant?: 'mark' | 'word' | 'glyph';
    /** Height in CSS pixels; the width follows the logo's proportions. */
    size?: number;
    /** Optional class on each letter (the splash animates them one by one). */
    letterClass?: string;
  }
  let { variant = 'mark', size = 24, letterClass = '' }: Props = $props();

  const art = $derived(variant === 'mark' ? MARK : WORDMARK);
</script>

{#if variant === 'glyph'}
  <svg viewBox="16 16 68 68" width={size} height={size} aria-hidden="true" focusable="false">{@html GLYPH}</svg>
{:else}
<svg
  viewBox="0 0 {art.width} {art.height}"
  height={size}
  width={(size * art.width) / art.height}
  fill="currentColor"
  fill-rule="evenodd"
  aria-hidden="true"
  focusable="false"
>
  {#each art.paths as d, i (i)}
    <path {d} class={letterClass} style:--i={i} />
  {/each}
</svg>
{/if}
