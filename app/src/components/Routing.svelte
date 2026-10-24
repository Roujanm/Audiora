<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';

  const strips = $derived(engine.info?.strips ?? []);
  const buses = $derived(engine.info?.buses ?? []);

  function toggle(s: number, b: number) {
    const path = `strip/${s}/bus_mask`;
    engine.set(path, engine.get(path, 1) ^ (1 << b));
  }
</script>

<div class="matrix" style:--cols={buses.length}>
  <div class="corner muted">Source ↓ &nbsp; Output →</div>
  {#each buses as b, bi (bi)}
    <div class="colhead" title={b.name}>
      <strong>{bi === 0 ? 'Master' : `Bus ${bi + 1}`}</strong>
      <span class="muted">{b.name}</span>
    </div>
  {/each}

  {#each strips as s, si (si)}
    <div class="rowhead" title={s.name}>
      <strong>{s.kind === 'input' ? `Input ${si + 1}` : 'Tone'}</strong>
      <span class="muted">{s.name}</span>
    </div>
    {#each buses as _b, bi (bi)}
      {@const on = ((engine.get(`strip/${si}/bus_mask`, 1) >> bi) & 1) === 1}
      <button
        class="cell"
        class:on
        aria-pressed={on}
        aria-label="{s.name} to {buses[bi].name}"
        onclick={() => toggle(si, bi)}
      >
        {on ? '●' : ''}
      </button>
    {/each}
  {/each}
</div>

<style>
  .matrix {
    display: grid;
    grid-template-columns: minmax(180px, 1.2fr) repeat(var(--cols), minmax(120px, 1fr));
    gap: 6px;
  }

  .corner {
    font-size: 11px;
    align-self: end;
    padding: 0 4px 4px;
  }

  .colhead,
  .rowhead {
    display: flex;
    flex-direction: column;
    min-width: 0;
    padding: 4px 6px;
  }

  .colhead span,
  .rowhead span {
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .cell {
    height: 38px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--line);
    background: var(--panel-2);
    color: var(--accent);
    font-size: 16px;
  }

  .cell:hover {
    background: var(--panel-3);
  }

  .cell.on {
    background: var(--accent-soft);
    border-color: var(--accent);
  }
</style>
