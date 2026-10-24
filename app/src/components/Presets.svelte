<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';

  let selected = $state('');
  let naming = $state(false);
  let newName = $state('');
  let note = $state('');

  const valid = $derived(/^[A-Za-z0-9 _-]{1,64}$/.test(newName.trim()) && !newName.trim().startsWith('.'));

  async function run(fn: () => Promise<void>, ok: string) {
    try {
      await fn();
      note = ok;
    } catch (e) {
      note = String(e instanceof Error ? e.message : e);
    }
    setTimeout(() => (note = ''), 2500);
  }

  async function save() {
    const name = newName.trim();
    await run(async () => {
      await engine.savePreset(name);
      selected = name;
      naming = false;
      newName = '';
    }, `Saved “${name}”`);
  }
</script>

<div class="presets">
  {#if naming}
    <input
      type="text"
      bind:value={newName}
      placeholder="Preset name"
      aria-label="Preset name"
      maxlength="64"
      onkeydown={(e) => {
        if (e.key === 'Enter' && valid) void save();
        if (e.key === 'Escape') naming = false;
      }}
    />
    <button class="btn primary" disabled={!valid} onclick={save}>Save</button>
    <button class="btn" onclick={() => (naming = false)}>Cancel</button>
  {:else}
    <select bind:value={selected} aria-label="Presets">
      <option value="">{engine.presets.length ? 'Presets…' : 'No presets yet'}</option>
      {#each engine.presets as p (p)}
        <option value={p}>{p}</option>
      {/each}
    </select>
    <button class="btn" disabled={!selected} onclick={() => run(() => engine.loadPreset(selected), `Loaded “${selected}”`)}>Load</button>
    <button class="btn" onclick={() => (naming = true)}>Save as…</button>
    <button
      class="btn danger"
      disabled={!selected}
      title="Delete the selected preset"
      onclick={() => run(async () => { await engine.deletePreset(selected); selected = ''; }, 'Deleted')}
    >
      Delete
    </button>
  {/if}
  {#if note}<span class="note muted" role="status">{note}</span>{/if}
</div>

<style>
  .presets {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  select {
    min-width: 150px;
  }

  input {
    width: 180px;
  }

  .note {
    font-size: 12px;
    margin-left: 4px;
  }
</style>
