<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import type { HwControl } from '../lib/protocol.ts';

  const info = $derived(engine.hwInfo);
  const groups = $derived(
    engine.hwControls.reduce<Record<string, HwControl[]>>((acc, c) => {
      (acc[c.group] ??= []).push(c);
      return acc;
    }, {}),
  );

  const meterFor = (group: string) => {
    const m = /^Input (\d+)$/.exec(group);
    return m ? (engine.hwMeters[`input/${+m[1] - 1}/peak`] ?? 0) : null;
  };
  const db = (lin: number) => (lin <= 1e-6 ? -120 : 20 * Math.log10(lin));
</script>

{#if !info}
  <div class="empty card">
    <h2>No interface connected</h2>
    <p class="muted">
      Connect an R Interface to control its preamps, phantom power and monitoring from here. Any other audio
      interface still works as a normal input and output device.
    </p>
  </div>
{:else}
  <div class="head">
    <div>
      <h2>{info.model}</h2>
      <p class="muted num">serial {info.serial} · firmware {info.firmware} · {info.inputs} in / {info.outputs} out</p>
    </div>
    {#if info.simulated}
      <span class="chip warn" title="No real hardware is connected; these controls only change a software model.">SIMULATOR — not real hardware</span>
    {/if}
  </div>

  <div class="groups">
    {#each Object.entries(groups) as [name, controls] (name)}
      {@const level = meterFor(name)}
      <section class="group card">
        <h2>{name}</h2>
        {#if level !== null}
          <div class="mt" role="img" aria-label="Input level">
            <div class="mtfill" style:width="{Math.min(Math.max((db(level) + 60) / 60, 0), 1) * 100}%"></div>
          </div>
        {/if}
        {#each controls as c (c.path)}
          {#if c.kind.type === 'range'}
            <label class="ctl">
              <span>{c.label}</span>
              <input
                type="range"
                min={c.kind.min}
                max={c.kind.max}
                step={c.kind.step}
                value={c.value ?? c.kind.min}
                oninput={(e) => void engine.setHw(c.path, +e.currentTarget.value)}
              />
              <output class="num">{(c.value ?? 0).toFixed(c.kind.step < 1 ? 2 : 0)} {c.kind.unit}</output>
            </label>
          {:else if c.kind.type === 'toggle'}
            <div class="ctl">
              <span>{c.label}</span>
              <button
                class="sw"
                class:on={(c.value ?? 0) > 0.5}
                class:phantom={c.path.endsWith('/phantom')}
                aria-pressed={(c.value ?? 0) > 0.5}
                onclick={() => void engine.setHw(c.path, (c.value ?? 0) < 0.5)}
              >
                {(c.value ?? 0) > 0.5 ? 'On' : 'Off'}
              </button>
            </div>
          {:else}
            <label class="ctl">
              <span>{c.label}</span>
              <select value={String(c.value ?? 0)} onchange={(e) => void engine.setHw(c.path, +e.currentTarget.value)}>
                {#each c.kind.options as o, i (o)}
                  <option value={String(i)}>{o}</option>
                {/each}
              </select>
            </label>
          {/if}
        {/each}
      </section>
    {/each}
  </div>
{/if}

<style>
  .head {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 12px;
    margin-bottom: 12px;
  }

  .head h2 {
    margin: 0 0 2px;
    font-size: 14px;
    color: var(--text);
    text-transform: none;
    letter-spacing: 0;
  }

  .head p {
    margin: 0;
    font-size: 12px;
  }

  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 12px;
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 8px;
    min-width: 0;
  }

  .group h2 {
    margin: 0;
  }

  .ctl {
    display: grid;
    grid-template-columns: 96px minmax(0, 1fr) 64px;
    align-items: center;
    gap: 8px;
    font-size: 12px;
  }

  .ctl > span {
    color: var(--muted);
  }

  .ctl select {
    grid-column: 2 / 4;
    min-width: 0;
  }

  .ctl input[type='range'] {
    min-width: 0;
    width: 100%;
    margin: 0;
  }

  .ctl .sw {
    justify-self: start;
  }

  .ctl output {
    text-align: right;
    white-space: nowrap;
    font-size: 11px;
  }

  .sw {
    min-width: 52px;
    height: 26px;
    border-radius: 13px;
    border: 1px solid var(--line);
    background: var(--bg);
    color: var(--muted);
    font-weight: 600;
    font-size: 11px;
  }

  .sw.on {
    background: var(--accent);
    border-color: var(--accent);
    color: #06101d;
  }

  .sw.phantom.on {
    background: var(--bad);
    border-color: var(--bad);
    color: #1b0508;
  }

  .mt {
    height: 8px;
    border-radius: 4px;
    background: var(--bg);
    border: 1px solid var(--line);
    overflow: hidden;
  }

  .mtfill {
    height: 100%;
    background: linear-gradient(90deg, var(--ok), var(--warn) 80%, var(--bad));
  }

  .empty p {
    margin: 0;
    max-width: 52ch;
  }
</style>
