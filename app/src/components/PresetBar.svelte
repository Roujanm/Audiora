<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';
  import PresetPicker from './PresetPicker.svelte';

  interface Props {
    channel: string;
    color: string;
  }
  let { channel, color }: Props = $props();

  const lists = $derived(engine.chPresets[channel] ?? { factory: [], user: [], favorites: [] });
  const current = $derived(engine.activePreset[channel] ?? '');

  let naming = $state(false);
  let name = $state('');
  const valid = $derived(name.trim().length > 0 && name.trim().length <= 40);

  async function run(fn: () => Promise<void>, ok: string) {
    try {
      await fn();
      toast(ok, 'ok');
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  const save = () =>
    run(async () => {
      const n = name.trim();
      await engine.saveChannelPreset(channel, n);
      naming = false;
      name = '';
    }, 'Preset saved');
</script>

<div class="bar panel" style:--c={color}>
  <PresetPicker {channel} {color} />

  <div class="favs" role="group" aria-label="Favorite presets">
    {#each lists.favorites as f (f)}
      <button class="fav" class:on={f === current} aria-pressed={f === current} onclick={() => run(() => engine.loadChannelPreset(channel, f), `Loaded “${f}”`)}>
        <Icon name="star" size={11} /> {f}
      </button>
    {:else}
      <span class="muted none">Star a preset to pin it here for one-click access.</span>
    {/each}
  </div>

  <div class="end">
    {#if naming}
      <form
        class="name"
        onsubmit={(e) => {
          e.preventDefault();
          if (valid) void save();
        }}
      >
        <input type="text" bind:value={name} placeholder="Preset name" maxlength="40" aria-label="Preset name" />
        <button class="btn sm primary" type="submit" disabled={!valid}>Save</button>
        <button class="btn sm ghost" type="button" onclick={() => (naming = false)}>Cancel</button>
      </form>
    {:else}
      <button class="btn sm" onclick={() => run(() => engine.loadChannelPreset(channel, 'Flat'), 'Reset to flat')} title="Load the Flat preset"><Icon name="refresh" size={13} /> Reset</button>
      <button
        class="btn sm"
        onclick={() => {
          naming = true;
          name = lists.user.includes(current) ? current : '';
        }}
      ><Icon name="save" size={13} /> Save as…</button>
    {/if}
  </div>
</div>

<style>
  .bar {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    flex-wrap: wrap;
  }

  .favs {
    flex: 1;
    min-width: 160px;
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
    align-items: center;
  }

  .fav {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px solid var(--line-2);
    background: var(--bg-2);
    color: var(--text-2);
    font-size: 12px;
    font-weight: 600;
    transition: all 0.12s;
  }

  .fav :global(svg) {
    color: #ffd24d;
  }

  .fav:hover {
    color: var(--text);
    border-color: var(--line-3);
  }

  .fav.on {
    color: var(--text);
    border-color: color-mix(in oklab, var(--c) 60%, transparent);
    background: color-mix(in oklab, var(--c) 16%, transparent);
  }

  .none {
    font-size: 12px;
  }

  .end {
    display: flex;
    gap: 8px;
  }

  .name {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .name input {
    width: 170px;
    height: 28px;
  }
</style>
