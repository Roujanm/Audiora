<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { linToDb } from '../lib/dsp.ts';
  import type { HwControl } from '../lib/protocol.ts';
  import Icon from './Icon.svelte';
  import Knob from './Knob.svelte';
  import LedMeter from './LedMeter.svelte';
  import Segmented from './Segmented.svelte';
  import Switch from './Switch.svelte';

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

  const ranges = (cs: HwControl[]) => cs.filter((c) => c.kind.type === 'range');
  const others = (cs: HwControl[]) => cs.filter((c) => c.kind.type !== 'range');

  function fmt(c: HwControl, v: number) {
    if (c.kind.type !== 'range') return String(v);
    const unit = c.kind.unit;
    const d = c.kind.step < 1 ? (c.kind.step < 0.1 ? 2 : 1) : 0;
    return `${v > 0 && c.kind.min < 0 ? '+' : ''}${v.toFixed(d)}${unit ? ` ${unit}` : ''}`;
  }
</script>

{#if !info}
  <section class="empty panel pad">
    <span class="big"><Icon name="interface" size={30} /></span>
    <div>
      <h2>No audio interface connected</h2>
      <p class="muted">
        Plug in an R Interface to control its preamps, phantom power and monitoring from here. Any other audio
        interface still works as a normal input and output device.
      </p>
    </div>
  </section>
{:else}
  <section class="plate panel">
    <div class="id">
      <span class="logo"><Icon name="interface" size={22} /></span>
      <div>
        <h2>{info.model}</h2>
        <p class="muted num">serial {info.serial} · firmware {info.firmware} · {info.inputs} in / {info.outputs} out</p>
      </div>
    </div>
    {#if info.simulated}
      <span class="chip warn sim" title="No real hardware is connected; these controls only change a software model.">
        <Icon name="alert" size={13} /> SIMULATOR — not real hardware
      </span>
    {/if}
  </section>

  <div class="groups">
    {#each Object.entries(groups) as [name, controls] (name)}
      {@const level = meterFor(name)}
      <section class="group panel">
        <header>
          <h3 class="eyebrow">{name}</h3>
          {#if level !== null}<span class="num lv">{linToDb(level) > -100 ? linToDb(level).toFixed(0) : '−∞'} dB</span>{/if}
        </header>

        {#if level !== null}
          <LedMeter orientation="horizontal" scale={false} thickness={6} levels={[linToDb(level)]} />
        {/if}

        {#if ranges(controls).length}
          <div class="knobs">
            {#each ranges(controls) as c (c.path)}
              {#if c.kind.type === 'range'}
                <Knob
                  label={c.label.replace(/\s*\(.*\)$/, '')}
                  size={64}
                  min={c.kind.min}
                  max={c.kind.max}
                  step={c.kind.step}
                  value={c.value ?? c.kind.min}
                  format={(v) => fmt(c, v)}
                  onchange={(v) => void engine.setHw(c.path, v)}
                />
              {/if}
            {/each}
          </div>
        {/if}

        {#if others(controls).length}
          <div class="rows">
            {#each others(controls) as c (c.path)}
              <div class="ctl">
                <span>{c.label}</span>
                {#if c.kind.type === 'toggle'}
                  <span class="sw" class:phantom={c.path.endsWith('/phantom')}>
                    <Switch checked={(c.value ?? 0) > 0.5} label={c.label} onchange={(v) => void engine.setHw(c.path, v)} />
                  </span>
                {:else if c.kind.type === 'choice'}
                  <Segmented
                    size="sm"
                    label={c.label}
                    value={String(c.value ?? 0)}
                    options={c.kind.options.map((o, i) => ({ id: String(i), label: o }))}
                    onchange={(id) => void engine.setHw(c.path, +id)}
                  />
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </section>
    {/each}
  </div>
{/if}

<style>
  .empty {
    display: flex;
    align-items: center;
    gap: 20px;
    max-width: 640px;
  }

  .empty h2 {
    font-size: 16px;
    font-weight: 650;
    margin-bottom: 4px;
  }

  .big {
    flex: none;
    width: 58px;
    height: 58px;
    border-radius: 16px;
    display: grid;
    place-items: center;
    color: var(--muted);
    background: var(--bg-3);
    border: 1px solid var(--line-2);
  }

  .plate {
    display: flex;
    justify-content: space-between;
    align-items: center;
    gap: 16px;
    padding: 16px 20px;
    margin-bottom: 16px;
    background:
      linear-gradient(90deg, color-mix(in oklab, var(--accent) 9%, transparent), transparent 55%),
      linear-gradient(180deg, var(--bg-2), var(--bg-1));
  }

  .id {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .logo {
    width: 44px;
    height: 44px;
    border-radius: 13px;
    display: grid;
    place-items: center;
    color: var(--accent-hi);
    background: var(--accent-soft);
    border: 1px solid color-mix(in oklab, var(--accent) 35%, transparent);
  }

  .id h2 {
    font-size: 16px;
    font-weight: 650;
  }

  .id p {
    font-size: 12px;
  }

  .sim {
    height: 28px;
  }

  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
    gap: 16px;
    align-items: start;
  }

  .group {
    padding: 16px 18px 18px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .lv {
    font-size: 11px;
    color: var(--muted);
  }

  .knobs {
    display: flex;
    flex-wrap: wrap;
    gap: 14px 22px;
    justify-content: space-around;
  }

  .rows {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-top: 12px;
    border-top: 1px solid var(--line);
  }

  .ctl {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .ctl > span:first-child {
    color: var(--text-2);
    font-weight: 550;
  }

  .sw.phantom :global(.switch.on) {
    background: linear-gradient(180deg, #ff8794, var(--bad));
    box-shadow: 0 0 14px -2px var(--bad);
  }
</style>
