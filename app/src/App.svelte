<script lang="ts">
  import { engine } from './lib/engine.svelte.ts';
  import Strip from './components/Strip.svelte';
  import BusStrip from './components/BusStrip.svelte';
  import EqEditor from './components/EqEditor.svelte';
  import DynamicsPanel from './components/DynamicsPanel.svelte';
  import Routing from './components/Routing.svelte';
  import Recorder from './components/Recorder.svelte';
  import Hardware from './components/Hardware.svelte';
  import System from './components/System.svelte';
  import Presets from './components/Presets.svelte';

  engine.start();

  type Skin = 'play' | 'studio';
  type Tab = 'mixer' | 'processing' | 'record' | 'routing' | 'hardware' | 'system';

  function remembered<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
    try {
      const v = localStorage.getItem(key) as T | null;
      return v && allowed.includes(v) ? v : fallback;
    } catch {
      return fallback;
    }
  }

  let skin = $state<Skin>(remembered('audiora.skin', ['play', 'studio'], 'play'));
  let tab = $state<Tab>('mixer');
  let selected = $state(0);

  $effect(() => {
    try {
      localStorage.setItem('audiora.skin', skin);
    } catch {
      /* private mode: preference just is not remembered */
    }
  });

  // The play skin only has the mixer; leaving studio returns to it.
  $effect(() => {
    if (skin === 'play') tab = 'mixer';
  });

  const strips = $derived(engine.info?.strips ?? []);
  const buses = $derived(engine.info?.buses ?? []);
  $effect(() => {
    if (selected >= strips.length) selected = 0;
  });

  const tabs: { id: Tab; label: string }[] = [
    { id: 'mixer', label: 'Mixer' },
    { id: 'processing', label: 'EQ & Dynamics' },
    { id: 'record', label: 'Record' },
    { id: 'routing', label: 'Routing' },
    { id: 'hardware', label: 'Interface' },
    { id: 'system', label: 'System' },
  ];

  const s = $derived(engine.stats);
  const health = $derived(s.underruns + s.drift_drops + s.capture_drops);
</script>

<div class="app">
  <header class="top">
    <div class="brand">
      <span class="logo" aria-hidden="true">
        <svg viewBox="0 0 24 24" width="22" height="22"><path d="M3 14v-4M7 18V6M11 21V3M15 17V7M19 14v-4" stroke="currentColor" stroke-width="2.4" stroke-linecap="round" fill="none" /></svg>
      </span>
      <span class="name">AUDIORA</span>
      <span class="by muted">Roujan Digital Agency</span>
    </div>

    {#if skin === 'studio'}
      <nav class="tabs" aria-label="Sections">
        {#each tabs as t (t.id)}
          <button class="tab" class:active={tab === t.id} aria-current={tab === t.id ? 'page' : undefined} onclick={() => (tab = t.id)}>
            {t.label}
            {#if t.id === 'record' && engine.rec.recording}<i class="recdot" aria-label="recording"></i>{/if}
          </button>
        {/each}
      </nav>
    {/if}

    <div class="spacer"></div>
    {#if engine.connected}<Presets />{/if}

    <div class="skin" role="group" aria-label="Interface mode">
      <button class:active={skin === 'play'} aria-pressed={skin === 'play'} onclick={() => (skin = 'play')}>Play</button>
      <button class:active={skin === 'studio'} aria-pressed={skin === 'studio'} onclick={() => (skin = 'studio')}>Studio</button>
    </div>
  </header>

  <main>
    {#if !engine.connected}
      <div class="connect card" role="status">
        <div class="spin" aria-hidden="true"></div>
        <div>
          <h2>{engine.connecting ? 'Connecting to the Audiora engine…' : 'Engine not reachable'}</h2>
          <p class="muted">
            Start <code>audiora-svc</code> (the app does this automatically when installed). Retrying every second.
          </p>
          {#if engine.lastError}<p class="chip bad">{engine.lastError}</p>{/if}
        </div>
      </div>
    {:else if tab === 'mixer'}
      <div class="mixer">
        <div class="strips" role="group" aria-label="Input strips">
          {#each strips as _s, i (i)}
            <Strip index={i} advanced={skin === 'studio'} selected={selected === i} onselect={() => { selected = i; tab = 'processing'; }} />
          {/each}
        </div>
        <div class="divider"></div>
        <div class="buses" role="group" aria-label="Output buses">
          {#each buses as _b, i (i)}
            <BusStrip index={i} />
          {/each}
        </div>
      </div>
    {:else if tab === 'processing'}
      <div class="scroll">
        <div class="picker" role="tablist" aria-label="Strip">
          {#each strips as s2, i (i)}
            <button role="tab" aria-selected={selected === i} class:active={selected === i} onclick={() => (selected = i)}>
              {s2.kind === 'input' ? `Input ${i + 1}` : 'Tone'} <span class="muted">{s2.name}</span>
            </button>
          {/each}
        </div>
        <section class="card"><EqEditor strip={selected} /></section>
        <DynamicsPanel strip={selected} />
      </div>
    {:else if tab === 'record'}
      <div class="scroll"><section class="card"><Recorder /></section></div>
    {:else if tab === 'routing'}
      <div class="scroll"><section class="card"><h2>Routing matrix</h2><Routing /></section></div>
    {:else if tab === 'hardware'}
      <div class="scroll"><Hardware /></div>
    {:else}
      <div class="scroll"><System /></div>
    {/if}
  </main>

  <footer class="status">
    <span class="dot" class:on={engine.connected} aria-hidden="true"></span>
    <span>{engine.connected ? 'Engine running' : 'Offline'}</span>
    {#if engine.connected}
      <span class="chip" class:warn={health > 6}>{health === 0 ? 'No audio glitches' : `${health} glitch events`}</span>
      {#if engine.rec.recording}<span class="chip bad">● Recording</span>{/if}
      {#if engine.hwInfo?.simulated}<span class="chip warn">Interface simulator</span>{/if}
      {#if engine.lastError}<span class="chip bad" title={engine.lastError}>{engine.lastError.slice(0, 80)}</span>{/if}
    {/if}
    <span class="spacer"></span>
    <span class="muted">v0.1 · {engine.license?.state === 'valid' ? `${engine.license.edition} licence` : 'Play edition'}</span>
  </footer>
</div>

<style>
  .app {
    display: grid;
    grid-template-rows: auto 1fr auto;
    height: 100%;
  }

  .top {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 10px 16px;
    border-bottom: 1px solid var(--line);
    background: var(--panel);
    flex-wrap: wrap;
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .logo {
    display: grid;
    place-items: center;
    width: 32px;
    height: 32px;
    border-radius: 8px;
    background: linear-gradient(135deg, #3d8bff, #6c5cff);
    color: #fff;
  }

  .name {
    font-weight: 700;
    letter-spacing: 0.16em;
    font-size: 14px;
  }

  .by {
    font-size: 11px;
    padding-left: 10px;
    border-left: 1px solid var(--line);
  }

  .tabs {
    display: flex;
    gap: 2px;
  }

  @media (max-width: 1500px) {
    .by {
      display: none;
    }
  }

  .tab {
    position: relative;
    height: 30px;
    padding: 0 12px;
    background: none;
    border: 0;
    border-radius: var(--radius-sm);
    color: var(--muted);
  }

  .tab:hover {
    background: var(--panel-2);
    color: var(--text);
  }

  .tab.active {
    background: var(--accent-soft);
    color: var(--accent);
    font-weight: 600;
  }

  .recdot {
    display: inline-block;
    width: 7px;
    height: 7px;
    margin-left: 6px;
    border-radius: 50%;
    background: var(--rec);
    animation: pulse 1.2s infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.25;
    }
  }

  .spacer {
    flex: 1;
  }

  .skin {
    display: flex;
    padding: 2px;
    border-radius: var(--radius-sm);
    background: var(--bg);
    border: 1px solid var(--line);
  }

  .skin button {
    height: 26px;
    padding: 0 14px;
    border: 0;
    border-radius: 4px;
    background: none;
    color: var(--muted);
  }

  .skin button.active {
    background: var(--panel-3);
    color: var(--text);
    font-weight: 600;
  }

  main {
    min-height: 0;
    padding: 14px 16px;
    overflow: hidden;
  }

  .mixer {
    display: flex;
    gap: 14px;
    height: 100%;
    min-height: 0;
    overflow-x: auto;
    padding-bottom: 6px;
  }

  .strips,
  .buses {
    display: flex;
    gap: 10px;
    min-height: 0;
  }

  .strips > :global(*),
  .buses > :global(*) {
    height: 100%;
    max-height: 560px;
  }

  .divider {
    width: 1px;
    background: var(--line);
    flex: none;
  }

  .scroll {
    height: 100%;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: 12px;
    padding-right: 4px;
  }

  .picker {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .picker button {
    height: 30px;
    padding: 0 12px;
    border-radius: var(--radius-sm);
    border: 1px solid var(--line);
    background: var(--panel-2);
    max-width: 280px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .picker button.active {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .connect {
    display: flex;
    align-items: center;
    gap: 16px;
    max-width: 560px;
    margin: 12vh auto 0;
  }

  .connect h2 {
    color: var(--text);
    font-size: 15px;
    letter-spacing: 0;
    text-transform: none;
  }

  .connect p {
    margin: 0 0 8px;
  }

  code {
    font-family: var(--mono);
    background: var(--bg);
    padding: 1px 5px;
    border-radius: 4px;
  }

  .spin {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 3px solid var(--panel-3);
    border-top-color: var(--accent);
    animation: spin 0.9s linear infinite;
    flex: none;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .status {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 6px 16px;
    border-top: 1px solid var(--line);
    background: var(--panel);
    font-size: 12px;
  }

  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--bad);
  }

  .dot.on {
    background: var(--ok);
    box-shadow: 0 0 6px var(--ok);
  }
</style>
