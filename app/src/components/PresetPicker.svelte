<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';
  import Popover from './Popover.svelte';

  interface Props {
    channel: string;
    /** `chip` is the small trigger used on a mixer column, `field` the wide one on a page. */
    variant?: 'chip' | 'field';
    color?: string;
  }
  let { channel, variant = 'field', color = 'var(--accent)' }: Props = $props();

  const lists = $derived(engine.chPresets[channel] ?? { factory: [], user: [], favorites: [] });
  const strip = $derived(engine.channels?.channels.find((c) => c.key === channel)?.strip ?? -1);
  const isFlat = $derived(
    strip >= 0 &&
      [0, 1, 2, 3, 4, 5].every((b) => Math.abs(engine.get(`strip/${strip}/eq/${b}/gain_db`)) < 0.01) &&
      engine.get(`strip/${strip}/boost_db`) < 0.05 &&
      ['smart/enabled', 'denoise', 'gate/enabled', 'comp/enabled'].every((l) => engine.get(`strip/${strip}/${l}`) < 0.5),
  );
  const current = $derived(engine.activePreset[channel] ?? (isFlat ? 'Flat' : ''));
  const isFav = (n: string) => lists.favorites.includes(n);

  async function load(name: string, close: () => void) {
    try {
      await engine.loadChannelPreset(channel, name);
      close();
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  async function star(name: string, e: Event) {
    e.stopPropagation();
    try {
      await engine.favoriteChannelPreset(channel, name, !isFav(name));
    } catch (err) {
      toast(String(err instanceof Error ? err.message : err), 'error');
    }
  }

  async function remove(name: string, e: Event) {
    e.stopPropagation();
    try {
      await engine.deleteChannelPreset(channel, name);
      toast(`Deleted “${name}”`, 'ok');
    } catch (err) {
      toast(String(err instanceof Error ? err.message : err), 'error');
    }
  }
</script>

<Popover width={variant === 'chip' ? 224 : 290} align="start">
  {#snippet trigger({ open, toggle })}
    <button class="trig {variant}" class:open style:--c={color} onclick={toggle} aria-haspopup="listbox" aria-expanded={open} title="Choose a preset">
      {#if variant === 'field'}<span class="lbl">Preset</span>{/if}
      <span class="val">{current || 'Custom'}</span>
      <Icon name="chevron" size={13} />
    </button>
  {/snippet}

  {#snippet children({ close })}
    <div class="menu" role="listbox" aria-label="Presets">
      {#if lists.favorites.length}
        <p class="eyebrow">Favorites</p>
        {#each lists.favorites as n (n)}
          {@render item(n, lists.user.includes(n), close)}
        {/each}
      {/if}
      <p class="eyebrow">Built-in</p>
      {#each lists.factory as n (n)}
        {@render item(n, false, close)}
      {/each}
      {#if lists.user.length}
        <p class="eyebrow">My presets</p>
        {#each lists.user as n (n)}
          {@render item(n, true, close)}
        {/each}
      {/if}
    </div>
  {/snippet}
</Popover>

{#snippet item(n: string, deletable: boolean, close: () => void)}
  <div class="row" class:current={n === current}>
    <button class="pick" role="option" aria-selected={n === current} onclick={() => load(n, close)}>
      <span class="check">{#if n === current}<Icon name="check" size={13} stroke={2.2} />{/if}</span>
      <span class="n">{n}</span>
    </button>
    <button class="mini" class:on={isFav(n)} aria-label={isFav(n) ? `Remove ${n} from favorites` : `Add ${n} to favorites`} title="Favorite" onclick={(e) => star(n, e)}>
      <Icon name="star" size={13} />
    </button>
    {#if deletable}
      <button class="mini del" aria-label="Delete {n}" title="Delete" onclick={(e) => remove(n, e)}><Icon name="trash" size={13} /></button>
    {/if}
  </div>
{/snippet}

<style>
  .trig {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
    border: 1px solid var(--line-2);
    background: linear-gradient(180deg, var(--bg-3), var(--bg-2));
    color: var(--text);
    box-shadow: var(--shadow-1);
    transition: border-color 0.15s, background 0.15s;
  }

  .trig:hover,
  .trig.open {
    border-color: color-mix(in oklab, var(--c) 55%, transparent);
  }

  .trig.chip {
    width: 100%;
    height: 28px;
    padding: 0 8px 0 10px;
    border-radius: 8px;
    font-size: 11.5px;
    font-weight: 600;
  }

  .trig.field {
    height: 36px;
    padding: 0 12px;
    border-radius: var(--r-md);
    font-weight: 600;
    min-width: 230px;
  }

  .lbl {
    font-size: 10px;
    font-weight: 650;
    letter-spacing: 0.1em;
    text-transform: uppercase;
    color: var(--muted);
  }

  .val {
    flex: 1;
    min-width: 0;
    text-align: left;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .menu {
    max-height: 340px;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .eyebrow {
    padding: 8px 8px 4px;
    font-size: 9.5px;
  }

  .row {
    display: flex;
    align-items: center;
    border-radius: 8px;
  }

  .row:hover {
    background: var(--bg-4);
  }

  .row.current .n {
    color: var(--text);
    font-weight: 650;
  }

  .pick {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    height: 32px;
    padding: 0 6px;
    border: 0;
    background: none;
    color: var(--text-2);
    text-align: left;
  }

  .check {
    flex: none;
    width: 16px;
    color: var(--accent-hi);
  }

  .n {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mini {
    flex: none;
    width: 26px;
    height: 26px;
    margin-right: 2px;
    border: 0;
    border-radius: 6px;
    display: grid;
    place-items: center;
    background: none;
    color: var(--faint);
    padding: 0;
  }

  .mini:hover {
    color: var(--text);
    background: var(--bg-3);
  }

  .mini.on {
    color: #ffd24d;
  }

  .mini.del:hover {
    color: var(--bad);
  }
</style>
