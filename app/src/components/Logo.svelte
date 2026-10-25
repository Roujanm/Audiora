<script lang="ts">
  import { MARK, WORDMARK } from '../lib/logo.ts';

  interface Props {
    variant?: 'mark' | 'word';
    /** Height in CSS pixels; the width follows the logo's proportions. */
    size?: number;
    /** Optional class on each letter (the splash animates them one by one). */
    letterClass?: string;
  }
  let { variant = 'mark', size = 24, letterClass = '' }: Props = $props();

  const art = $derived(variant === 'mark' ? MARK : WORDMARK);
</script>

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
