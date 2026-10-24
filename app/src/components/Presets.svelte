<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import Icon from './Icon.svelte';
  import Popover from './Popover.svelte';

  let naming = $state(false);
  let newName = $state('');
  let note = $state('');
  let active = $state('');

  const valid = $derived(/^[A-Za-z0-9 _-]{1,64}$/.test(newName.trim()) && !newName.trim().startsWith('.'));

  async function run(fn: () => Promise<void>, ok: string) {
    try {
      await fn();
      note = ok;
    } catch (e) {
      note = String(e instanceof Error ? e.message : e);
    }
    setTimeout(() => (note = ''), 2600);
  }

  const load = (name: string, close: () => void) =>
    run(async () => {
      await engine.loadPreset(name);
      active = name;
      close();
    }, `Loaded “${name}”`);

  async function save() {
    const name = newName.trim();
    await run(async () => {
      await engine.savePreset(name);
      active = name;
      naming = false;
      newName = '';
    }, `Saved “${name}”`);
  }
</script>

<Popover width={290}>
  {#snippet trigger({ open, toggle })}
    <button class="btn trigger" class:open onclick={toggle} aria-haspopup="dialog" aria-expanded={open}>
      <Icon name="save" size={15} />
      <span class="name">{active || 'Presets'}</span>
      <span class="chev"><Icon name="chevron" size={13} /></span>
    </button>
  {/snippet}

  {#snippet children({ close })}
    <div class="list" role="listbox" aria-label="Presets">
      {#each engine.presets as p (p)}
        <div class="row" class:current={p === active}>
          <button class="load" role="option" aria-selected={p === active} onclick={() => load(p, close)}>
            <span class="check">{#if p === active}<Icon name="check" size={13} stroke={2.2} />{/if}</span>
            <span class="pname">{p}</span>
          </button>
          <button
            class="del"
            aria-label="Delete preset {p}"
            title="Delete"
            onclick={() => run(async () => { await engine.deletePreset(p); if (active === p) active = ''; }, 'Deleted')}
          >
            <Icon name="trash" size={14} />
          </button>
        </div>
      {:else}
        <p class="empty muted">No presets yet. Save your current mix to recall it with one click.</p>
      {/each}
    </div>

    <div class="foot">
      {#if naming}
        <input
          type="text"
          bind:value={newName}
          placeholder="Name this mix"
          aria-label="Preset name"
          maxlength="64"
          onkeydown={(e) => {
            if (e.key === 'Enter' && valid) void save();
            if (e.key === 'Escape') naming = false;
          }}
        />
        <button class="btn primary" disabled={!valid} onclick={save}>Save</button>
      {:else}
        <button class="btn newbtn" onclick={() => (naming = true)}><Icon name="plus" size={15} /> Save current mix…</button>
      {/if}
    </div>
    {#if note}<p class="note" role="status">{note}</p>{/if}
  {/snippet}
</Popover>

<style>
  .trigger {
    min-width: 150px;
    justify-content: space-between;
  }

  .trigger.open {
    border-color: var(--accent);
  }

  .name {
    max-width: 130px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chev {
    color: var(--muted);
    display: grid;
  }

  .list {
    display: flex;
    flex-direction: column;
    gap: 2px;
    max-height: 260px;
    overflow: auto;
  }

  .row {
    display: flex;
    align-items: center;
    border-radius: 8px;
  }

  .row:hover {
    background: var(--bg-4);
  }

  .row.current {
    background: var(--accent-soft);
  }

  .load {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 10px;
    border: 0;
    background: none;
    text-align: left;
    font-weight: 550;
  }

  .check {
    width: 14px;
    color: var(--accent-hi);
    display: grid;
  }

  .pname {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .del {
    width: 30px;
    height: 30px;
    margin-right: 3px;
    border: 0;
    border-radius: 7px;
    background: none;
    color: var(--faint);
    display: grid;
    place-items: center;
    opacity: 0;
  }

  .row:hover .del,
  .del:focus-visible {
    opacity: 1;
  }

  .del:hover {
    color: var(--bad);
    background: color-mix(in oklab, var(--bad) 12%, transparent);
  }

  .empty {
    padding: 14px 10px;
    font-size: 12px;
  }

  .foot {
    display: flex;
    gap: 6px;
    margin-top: 6px;
    padding-top: 8px;
    border-top: 1px solid var(--line);
  }

  .foot input {
    flex: 1;
    min-width: 0;
  }

  .newbtn {
    flex: 1;
    justify-content: flex-start;
  }

  .note {
    margin: 8px 4px 2px;
    font-size: 12px;
    color: var(--text-2);
  }
</style>
