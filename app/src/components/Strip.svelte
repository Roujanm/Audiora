<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { periodLabel, rateLabel, shortName, stripColor } from '../lib/theme.ts';
  import Fader from './Fader.svelte';
  import Icon from './Icon.svelte';
  import Knob from './Knob.svelte';
  import LedMeter from './LedMeter.svelte';
  import Pill from './Pill.svelte';

  interface Props {
    index: number;
    selected?: boolean;
    onselect?: () => void;
    /** Studio skin: show the processing summary and an edit button. */
    advanced?: boolean;
  }
  let { index, selected = false, onselect, advanced = false }: Props = $props();

  const node = $derived(engine.info?.strips[index]);
  const p = (leaf: string) => `strip/${index}/${leaf}`;
  const buses = $derived(engine.info?.buses ?? []);
  const mask = $derived(engine.get(p('bus_mask'), 1));
  const isInput = $derived(node?.kind === 'input');
  const color = $derived(stripColor(node, index));

  const on = (leaf: string) => engine.get(p(leaf)) > 0.5;
  const eqOn = $derived([0, 1, 2, 3].some((b) => Math.abs(engine.get(p(`eq/${b}/gain_db`))) >= 0.01));
  const tags = $derived([
    { id: 'NR', on: on('denoise'), show: isInput },
    { id: 'GATE', on: on('gate/enabled'), show: true },
    { id: 'EQ', on: eqOn, show: true },
    { id: 'COMP', on: on('comp/enabled'), show: true },
  ]);
  const gr = $derived(on('comp/enabled') ? engine.gauge(p('comp/gr_db')) : 0);

  const toggleBus = (b: number) => engine.set(p('bus_mask'), mask ^ (1 << b));
  const name = $derived(node ? (isInput ? shortName(node.name) : node.name) : '—');
</script>

<section class="strip panel" class:selected style:--c={color} aria-label={node?.name ?? `Strip ${index + 1}`}>
  <i class="topbar"></i>

  <header>
    <span class="badge"><Icon name={isInput ? 'mic' : 'wave'} size={15} /></span>
    <div class="meta">
      <h3 title={node?.name}>{name}</h3>
      <span class="sub num">
        {#if isInput}{rateLabel(node?.sample_rate)} · {periodLabel(node?.sample_rate, node?.period_frames)}{:else}Test signal{/if}
      </span>
    </div>
  </header>

  <div class="knobrow">
    <Knob
      label="Balance"
      size={50}
      min={-1}
      max={1}
      step={0.05}
      digits={2}
      bipolar
      reset={0}
      color={color}
      value={engine.get(p('balance'))}
      format={(v) => (Math.abs(v) < 0.025 ? 'C' : v < 0 ? `L${Math.round(-v * 100)}` : `R${Math.round(v * 100)}`)}
      onchange={(v) => engine.set(p('balance'), v)}
    />
  </div>

  <div class="pills">
    <Pill active={on('mute')} variant="mute" title="Mute" label="Mute {name}" onclick={() => engine.toggle(p('mute'))}>M</Pill>
    <Pill active={on('solo')} variant="solo" title="Solo" label="Solo {name}" onclick={() => engine.toggle(p('solo'))}>S</Pill>
    {#if isInput}
      <Pill active={on('denoise')} variant="ok" title="Noise suppression (48 kHz)" label="Noise suppression {name}" onclick={() => engine.toggle(p('denoise'))}>NR</Pill>
    {/if}
  </div>

  <div class="routes" role="group" aria-label="Send to outputs">
    <span class="eyebrow">Send</span>
    <div class="chips">
      {#each buses as bus, b (b)}
        <button
          class="route"
          class:on={(mask >> b) & 1}
          aria-pressed={((mask >> b) & 1) === 1}
          title="{(mask >> b) & 1 ? 'Sending to' : 'Send to'} {bus.name}"
          onclick={() => toggleBus(b)}
        >
          {b === 0 ? 'M' : b + 1}
        </button>
      {/each}
    </div>
  </div>

  <div class="main">
    <Fader value={engine.get(p('gain_db'))} {color} label="{name} level" onchange={(db) => engine.set(p('gain_db'), db)} />
    <LedMeter
      levels={[engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))]}
      holds={[engine.holdDb(p('peak_l')), engine.holdDb(p('peak_r'))]}
    />
  </div>

  <footer>
    <div class="tags" aria-label="Active processing">
      {#each tags.filter((t) => t.show) as t (t.id)}
        <span class="tag" class:on={t.on}>{t.id}{#if t.id === 'COMP' && gr > 0.3}<b class="num"> −{gr.toFixed(0)}</b>{/if}</span>
      {/each}
    </div>
    {#if advanced}
      <button class="edit" class:active={selected} onclick={onselect}>
        <Icon name="eq" size={14} /> {selected ? 'Editing' : 'Edit EQ / FX'}
      </button>
    {/if}
  </footer>
</section>

<style>
  .strip {
    width: 148px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding: 14px 14px 12px;
    min-height: 0;
    overflow: hidden;
    transition: border-color 0.2s;
  }

  .strip.selected {
    border-color: color-mix(in oklab, var(--c) 55%, transparent);
    box-shadow: var(--shadow-2), 0 0 0 1px color-mix(in oklab, var(--c) 25%, transparent);
  }

  .topbar {
    position: absolute;
    inset: 0 0 auto 0;
    height: 3px;
    background: var(--c);
    box-shadow: 0 0 18px 1px color-mix(in oklab, var(--c) 70%, transparent);
    border-radius: var(--r-lg) var(--r-lg) 0 0;
  }

  header {
    display: flex;
    gap: 9px;
    align-items: center;
    min-width: 0;
  }

  .badge {
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: color-mix(in oklab, var(--c) 14%, transparent);
    border: 1px solid color-mix(in oklab, var(--c) 28%, transparent);
  }

  .meta {
    min-width: 0;
    /* two name lines + sub line, so every strip lines up whatever the device is called */
    height: 46px;
    display: flex;
    flex-direction: column;
    justify-content: center;
  }

  h3 {
    font-size: 12.5px;
    font-weight: 650;
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    overflow-wrap: anywhere;
  }

  .sub {
    font-size: 10px;
    color: var(--faint);
    white-space: nowrap;
  }

  .knobrow {
    display: flex;
    justify-content: center;
    padding: 2px 0;
  }

  .pills {
    display: flex;
    gap: 5px;
  }

  .routes {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 6px;
  }

  .chips {
    display: flex;
    gap: 4px;
  }

  .route {
    width: 24px;
    height: 24px;
    border-radius: 6px;
    border: 1px solid var(--line-2);
    background: var(--bg-2);
    color: var(--faint);
    font: 700 10.5px var(--mono);
    transition: all 0.12s;
  }

  .route:hover {
    color: var(--text);
    border-color: var(--line-3);
  }

  .route.on {
    color: var(--c);
    background: color-mix(in oklab, var(--c) 16%, transparent);
    border-color: color-mix(in oklab, var(--c) 50%, transparent);
    box-shadow: 0 0 10px -2px color-mix(in oklab, var(--c) 60%, transparent);
  }

  .main {
    display: flex;
    gap: 8px;
    flex: 1;
    min-height: 170px;
    justify-content: center;
  }

  footer {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .tags {
    display: flex;
    flex-wrap: wrap;
    align-content: flex-start;
    gap: 4px;
    min-height: 38px;
  }

  .tag {
    font: 700 8.5px var(--font);
    letter-spacing: 0.08em;
    padding: 2px 5px;
    border-radius: 4px;
    color: var(--faint);
    background: var(--bg-0);
    border: 1px solid var(--line);
  }

  .tag.on {
    color: var(--c);
    border-color: color-mix(in oklab, var(--c) 40%, transparent);
    background: color-mix(in oklab, var(--c) 12%, transparent);
  }

  .tag b {
    font-weight: 700;
    color: var(--warn);
  }

  .edit {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    height: 28px;
    border-radius: 8px;
    border: 1px solid var(--line-2);
    background: var(--bg-2);
    color: var(--text-2);
    font-size: 11px;
    font-weight: 600;
  }

  .edit:hover {
    background: var(--bg-3);
    color: var(--text);
  }

  .edit.active {
    color: var(--c);
    border-color: color-mix(in oklab, var(--c) 45%, transparent);
  }

  /* short windows: trade a little air for keeping every control visible */
  @media (max-height: 780px) {
    .strip {
      gap: 8px;
      padding: 12px 12px 10px;
    }

    .main {
      min-height: 120px;
    }

    .knobrow :global(.label) {
      display: none;
    }

    .knobrow :global(.dial) {
      transform: scale(0.82);
      margin: -4px 0;
    }
  }
</style>
