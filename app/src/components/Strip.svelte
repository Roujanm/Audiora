<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import Fader from './Fader.svelte';
  import Meter from './Meter.svelte';

  interface Props {
    index: number;
    selected?: boolean;
    onselect?: () => void;
    /** Studio skin shows the processing indicators. */
    advanced?: boolean;
  }
  let { index, selected = false, onselect, advanced = false }: Props = $props();

  const node = $derived(engine.info?.strips[index]);
  const p = (leaf: string) => `strip/${index}/${leaf}`;
  const buses = $derived(engine.info?.buses ?? []);
  const mask = $derived(engine.get(p('bus_mask'), 1));
  const isInput = $derived(node?.kind === 'input');
  const fxOn = $derived(
    engine.get(p('gate/enabled')) > 0.5 ||
      engine.get(p('comp/enabled')) > 0.5 ||
      engine.get(p('denoise')) > 0.5 ||
      [0, 1, 2, 3].some((b) => Math.abs(engine.get(p(`eq/${b}/gain_db`))) >= 0.01),
  );
  const bal = $derived(engine.get(p('balance')));

  function toggleBus(b: number) {
    engine.set(p('bus_mask'), mask ^ (1 << b));
  }
</script>

<section class="strip card" class:selected aria-label={node?.name ?? `Strip ${index + 1}`}>
  <header>
    <span class="kind" class:tone={node?.kind === 'tone'}>{node?.kind === 'input' ? `IN ${index + 1}` : 'TONE'}</span>
    <h3 title={node?.name}>{node?.name ?? '—'}</h3>
  </header>

  <label class="bal">
    <span class="muted">Balance</span>
    <input
      type="range"
      min="-1"
      max="1"
      step="0.05"
      value={bal}
      aria-label="Balance"
      oninput={(e) => engine.set(p('balance'), +e.currentTarget.value)}
      ondblclick={() => engine.set(p('balance'), 0)}
    />
  </label>

  <div class="btns">
    <button class="tog mute" class:on={engine.get(p('mute')) > 0.5} onclick={() => engine.toggle(p('mute'))} aria-pressed={engine.get(p('mute')) > 0.5}>M</button>
    <button class="tog solo" class:on={engine.get(p('solo')) > 0.5} onclick={() => engine.toggle(p('solo'))} aria-pressed={engine.get(p('solo')) > 0.5}>S</button>
    {#if isInput}
      <button class="tog nr" class:on={engine.get(p('denoise')) > 0.5} onclick={() => engine.toggle(p('denoise'))} aria-pressed={engine.get(p('denoise')) > 0.5} title="Noise suppression (48 kHz)">NR</button>
    {/if}
  </div>

  <div class="routes" role="group" aria-label="Send to buses">
    {#each buses as _b, b (b)}
      <button class="route" class:on={(mask >> b) & 1} onclick={() => toggleBus(b)} aria-pressed={((mask >> b) & 1) === 1} title={`Send to ${buses[b].name}`}>
        {b + 1}
      </button>
    {/each}
  </div>

  <div class="main">
    <Fader value={engine.get(p('gain_db'))} label="{node?.name ?? 'Strip'} level" onchange={(db) => engine.set(p('gain_db'), db)} />
    <Meter
      levels={[engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))]}
      holds={[engine.holdDb(p('peak_l')), engine.holdDb(p('peak_r'))]}
    />
  </div>

  {#if advanced}
    <button class="fx" class:on={fxOn} onclick={onselect}>{selected ? 'Editing' : 'EQ · FX'}</button>
  {/if}
</section>

<style>
  .strip {
    width: 124px;
    flex: none;
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 10px;
    min-height: 0;
  }

  .strip.selected {
    border-color: var(--accent);
    box-shadow: 0 0 0 1px var(--accent-soft) inset;
  }

  header {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .kind {
    align-self: flex-start;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.08em;
    padding: 1px 6px;
    border-radius: 3px;
    background: var(--accent-soft);
    color: var(--accent);
  }

  .kind.tone {
    background: rgba(255, 180, 84, 0.14);
    color: var(--warn);
  }

  h3 {
    margin: 0;
    font-size: 12px;
    font-weight: 600;
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    min-height: 30px;
  }

  .bal {
    display: flex;
    flex-direction: column;
    font-size: 10px;
  }

  .bal input {
    width: 100%;
    margin: 2px 0 0;
  }

  .btns,
  .routes {
    display: flex;
    gap: 4px;
  }

  .tog,
  .route {
    flex: 1;
    height: 24px;
    border-radius: 4px;
    border: 1px solid var(--line);
    background: var(--panel-2);
    font-size: 11px;
    font-weight: 700;
    color: var(--muted);
  }

  .tog:hover,
  .route:hover {
    border-color: #3a4757;
  }

  .tog.mute.on {
    background: var(--bad);
    border-color: var(--bad);
    color: #1b0508;
  }

  .tog.solo.on {
    background: var(--warn);
    border-color: var(--warn);
    color: #1d1200;
  }

  .tog.nr.on {
    background: var(--ok);
    border-color: var(--ok);
    color: #04170e;
  }

  .route.on {
    background: var(--accent-soft);
    border-color: var(--accent);
    color: var(--accent);
  }

  .main {
    display: flex;
    gap: 6px;
    flex: 1;
    min-height: 150px;
  }

  .fx {
    height: 26px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--line);
    background: var(--panel-2);
    color: var(--muted);
    font-size: 11px;
  }

  .fx.on {
    color: var(--accent);
    border-color: var(--accent-soft);
  }

  .fx:hover {
    background: var(--panel-3);
  }
</style>
