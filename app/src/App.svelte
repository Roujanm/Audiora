<script lang="ts">
  import { engine } from './lib/engine.svelte.ts';
  import { ACCENTS, periodLabel, rateLabel, type AccentId } from './lib/theme.ts';
  import Strip from './components/Strip.svelte';
  import BusStrip from './components/BusStrip.svelte';
  import EqEditor from './components/EqEditor.svelte';
  import DynamicsPanel from './components/DynamicsPanel.svelte';
  import Routing from './components/Routing.svelte';
  import Recorder from './components/Recorder.svelte';
  import Hardware from './components/Hardware.svelte';
  import System from './components/System.svelte';
  import Apps from './components/Apps.svelte';
  import Presets from './components/Presets.svelte';
  import Icon, { type IconName } from './components/Icon.svelte';
  import Popover from './components/Popover.svelte';
  import Segmented from './components/Segmented.svelte';

  engine.start();

  type Skin = 'play' | 'studio';
  type Tab = 'mixer' | 'apps' | 'processing' | 'record' | 'routing' | 'hardware' | 'system';

  function remembered<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
    try {
      const v = localStorage.getItem(key) as T | null;
      return v && allowed.includes(v) ? v : fallback;
    } catch {
      return fallback;
    }
  }

  const accentIds = ACCENTS.map((a) => a.id);
  let skin = $state<Skin>(remembered('audiora.skin', ['play', 'studio'], 'play'));
  let accent = $state<AccentId>(remembered('audiora.accent', accentIds, 'azure'));
  let tab = $state<Tab>('mixer');
  let selected = $state(0);

  $effect(() => {
    document.documentElement.dataset.accent = accent;
    try {
      localStorage.setItem('audiora.skin', skin);
      localStorage.setItem('audiora.accent', accent);
    } catch {
      /* private mode: preferences just are not remembered */
    }
  });

  const NAV: { id: Tab; label: string; icon: IconName; studio: boolean; title: string }[] = [
    { id: 'mixer', label: 'Mixer', icon: 'mixer', studio: false, title: 'Mixer' },
    { id: 'apps', label: 'Apps', icon: 'apps', studio: false, title: 'Applications' },
    { id: 'processing', label: 'EQ · FX', icon: 'eq', studio: true, title: 'EQ & Dynamics' },
    { id: 'record', label: 'Record', icon: 'record', studio: true, title: 'Recorder' },
    { id: 'routing', label: 'Routing', icon: 'routing', studio: true, title: 'Routing' },
    { id: 'hardware', label: 'Interface', icon: 'interface', studio: true, title: 'Audio interface' },
    { id: 'system', label: 'System', icon: 'system', studio: false, title: 'System' },
  ];

  const visibleNav = $derived(NAV.filter((n) => skin === 'studio' || !n.studio));
  $effect(() => {
    if (!visibleNav.some((n) => n.id === tab)) tab = 'mixer';
  });

  const strips = $derived(engine.info?.strips ?? []);
  const buses = $derived(engine.info?.buses ?? []);
  $effect(() => {
    if (selected >= strips.length) selected = 0;
  });

  const current = $derived(NAV.find((n) => n.id === tab)!);
  const master = $derived(buses[0]);
  const s = $derived(engine.stats);
  const glitches = $derived(s.underruns + s.drift_drops + s.capture_drops + s.device_errors);
  const subtitle = $derived(
    tab === 'mixer'
      ? `${strips.filter((x) => x.kind === 'input').length} inputs · ${buses.length} output${buses.length === 1 ? '' : 's'}`
      : tab === 'apps'
        ? 'Volume per application, like the Windows mixer'
        : tab === 'processing'
          ? 'Per-channel equalizer, gate and compressor'
          : tab === 'record'
            ? '32-bit float WAV, multitrack'
            : tab === 'routing'
              ? 'Which input goes to which output'
              : tab === 'hardware'
                ? 'Preamps, phantom power, monitoring'
                : 'Devices, audio health and licence',
  );
</script>

<div class="app">
  <nav class="rail" aria-label="Sections">
    <div class="logo" aria-label="Audiora">
      <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true"><path d="M3 14v-4M7 18V6M11 21V3M15 17V7M19 14v-4" stroke="#fff" stroke-width="2.4" stroke-linecap="round" fill="none" /></svg>
    </div>
    <div class="items">
      {#each visibleNav as n (n.id)}
        <button class="item" class:active={tab === n.id} aria-current={tab === n.id ? 'page' : undefined} onclick={() => (tab = n.id)}>
          <span class="ico"><Icon name={n.icon} size={20} />{#if n.id === 'record' && engine.rec.recording}<i class="recdot" aria-label="recording"></i>{/if}</span>
          <span class="lbl">{n.label}</span>
        </button>
      {/each}
    </div>
    <span class="brand-foot">Roujan<br />Digital</span>
  </nav>

  <div class="shell">
    <header class="top">
      <div class="title">
        <h1>{current.title}</h1>
        <p class="muted">{subtitle}</p>
      </div>

      <div class="spacer"></div>

      {#if engine.connected && master}
        <div class="status-pill" title="Master output">
          <span class="dot"></span>
          <span class="state">Engine running</span>
          <span class="sep"></span>
          <span class="num">{rateLabel(master.sample_rate)} · {periodLabel(master.sample_rate, master.period_frames)}</span>
          {#if master.exclusive}<span class="chip accent">EXCLUSIVE</span>{:else if master.low_latency}<span class="chip accent">LOW LATENCY</span>{/if}
        </div>
      {/if}

      {#if engine.connected}<Presets />{/if}

      <Popover width={196}>
        {#snippet trigger({ open, toggle })}
          <button class="btn ghost icon" class:open aria-label="Accent colour" title="Accent colour" onclick={toggle}>
            <Icon name="palette" size={18} />
          </button>
        {/snippet}
        {#snippet children({ close })}
          <p class="eyebrow pop-title">Accent colour</p>
          <div class="swatches">
            {#each ACCENTS as a (a.id)}
              <button
                class="swatch"
                class:on={accent === a.id}
                style:--s={a.swatch}
                aria-label={a.label}
                aria-pressed={accent === a.id}
                title={a.label}
                onclick={() => {
                  accent = a.id;
                  close();
                }}
              ></button>
            {/each}
          </div>
        {/snippet}
      </Popover>

      <Segmented
        label="Interface mode"
        value={skin}
        options={[
          { id: 'play', label: 'Play' },
          { id: 'studio', label: 'Studio' },
        ]}
        onchange={(id) => (skin = id as Skin)}
      />
    </header>

    <main>
      {#if !engine.connected}
        <div class="connect panel pad" role="status">
          <div class="pulse" aria-hidden="true"><i></i><i></i><i></i></div>
          <div>
            <h2>{engine.connecting ? 'Connecting to the audio engine…' : 'Engine not reachable'}</h2>
            <p class="muted">
              The app starts <code>audiora-svc</code> for you when installed. Retrying every second.
            </p>
            {#if engine.lastError}<p class="chip bad err">{engine.lastError}</p>{/if}
          </div>
        </div>
      {:else if tab === 'mixer'}
        <div class="mixer">
          <section class="group inputs" aria-label="Inputs">
            <h2 class="eyebrow">Inputs</h2>
            <div class="row">
              {#each strips as _s, i (i)}
                <Strip index={i} advanced={skin === 'studio'} selected={selected === i && skin === 'studio'} onselect={() => { selected = i; tab = 'processing'; }} />
              {/each}
            </div>
          </section>
          <div class="divider"></div>
          <section class="group outputs" aria-label="Outputs">
            <h2 class="eyebrow">Outputs</h2>
            <div class="row">
              {#each buses as _b, i (i)}
                <BusStrip index={i} />
              {/each}
            </div>
          </section>
        </div>
      {:else if tab === 'apps'}
        <div class="scroll"><Apps /></div>
      {:else if tab === 'processing'}
        <div class="scroll"><div class="proc"><nav class="picker" aria-label="Channel">
          {#each strips as s2, i (i)}
            <button class:active={selected === i} aria-pressed={selected === i} onclick={() => (selected = i)}>
              <span class="n">{s2.kind === 'input' ? i + 1 : 'T'}</span>
              <span class="t">{s2.kind === 'input' ? s2.name.replace(/\s*\(.*\)\s*$/, '') : 'Tone generator'}</span>
            </button>
          {/each}
        </nav>
        <div class="stack"><EqEditor strip={selected} /><DynamicsPanel strip={selected} /></div></div></div>
      {:else if tab === 'record'}
        <div class="scroll"><Recorder /></div>
      {:else if tab === 'routing'}
        <div class="scroll"><Routing /></div>
      {:else if tab === 'hardware'}
        <div class="scroll"><Hardware /></div>
      {:else}
        <div class="scroll"><System /></div>
      {/if}
    </main>

    <footer class="foot">
      {#if engine.connected}
        <span class="chip" class:warn={glitches > 6} title="Underruns, drift drops, dropped blocks and device errors since start">
          {glitches === 0 ? 'No audio glitches' : `${glitches} glitch events`}
        </span>
        {#if engine.rec.recording}<span class="chip bad">● Recording</span>{/if}
        {#if engine.hwInfo?.simulated}<span class="chip warn">Interface simulator</span>{/if}
        {#if engine.lastError}<span class="chip bad" title={engine.lastError}>{engine.lastError.slice(0, 90)}</span>{/if}
      {:else}
        <span class="chip bad">Offline</span>
      {/if}
      <span class="spacer"></span>
      <span class="muted ver">Audiora 0.1 · {engine.license?.state === 'valid' ? `${engine.license.edition} licence` : 'Play edition'}</span>
    </footer>
  </div>
</div>

<style>
  .app {
    display: grid;
    grid-template-columns: 84px 1fr;
    height: 100%;
  }

  /* ----- rail */
  .rail {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 22px;
    padding: 18px 0 14px;
    background: linear-gradient(180deg, var(--bg-1), var(--bg-0));
    border-right: 1px solid var(--line);
  }

  .logo {
    width: 44px;
    height: 44px;
    display: grid;
    place-items: center;
    border-radius: 14px;
    background: linear-gradient(135deg, var(--accent), color-mix(in oklab, var(--accent) 45%, #7b5cff));
    box-shadow: 0 8px 24px -8px var(--accent-glow), 0 1px 0 rgba(255, 255, 255, 0.35) inset;
  }

  .items {
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 100%;
    padding: 0 10px;
    flex: 1;
  }

  .item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 4px;
    padding: 9px 0 7px;
    border: 0;
    border-radius: 12px;
    background: none;
    color: var(--muted);
    position: relative;
    transition: color 0.15s, background 0.15s;
  }

  .item:hover {
    color: var(--text);
    background: var(--bg-2);
  }

  .item.active {
    color: var(--accent-hi);
    background: var(--accent-soft);
  }

  .item.active::before {
    content: '';
    position: absolute;
    left: -10px;
    top: 14px;
    bottom: 14px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--accent);
    box-shadow: 0 0 12px var(--accent-glow);
  }

  .ico {
    position: relative;
    display: grid;
  }

  .lbl {
    font-size: 10.5px;
    font-weight: 600;
  }

  .recdot {
    position: absolute;
    top: -2px;
    right: -5px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--rec);
    box-shadow: 0 0 8px var(--rec);
    animation: pulse 1.2s infinite;
  }

  @keyframes pulse {
    50% {
      opacity: 0.3;
    }
  }

  .brand-foot {
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.16em;
    text-transform: uppercase;
    color: var(--faint);
    text-align: center;
    line-height: 1.5;
  }

  /* ----- shell */
  .shell {
    display: grid;
    grid-template-rows: auto 1fr auto;
    min-width: 0;
    min-height: 0;
  }

  .top {
    position: relative;
    z-index: 20; /* popovers open over the page below */
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 14px 22px;
    border-bottom: 1px solid var(--line);
    background: color-mix(in oklab, var(--bg-1) 70%, transparent);
    backdrop-filter: blur(10px);
  }

  .title {
    min-width: 0;
  }

  .title p {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .title h1 {
    font-size: 19px;
    font-weight: 700;
    letter-spacing: -0.015em;
    line-height: 1.15;
  }

  .title p {
    font-size: 12px;
  }

  .spacer {
    flex: 1;
  }

  .status-pill {
    flex: none;
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: 9px;
    height: 32px;
    padding: 0 12px;
    border-radius: 999px;
    background: var(--bg-2);
    border: 1px solid var(--line-2);
    font-size: 12px;
    color: var(--text-2);
  }

  .status-pill .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--ok);
    box-shadow: 0 0 10px var(--ok);
  }

  .status-pill .sep {
    width: 1px;
    height: 14px;
    background: var(--line-2);
  }

  .btn.icon {
    width: 32px;
    padding: 0;
  }

  .btn.icon.open {
    background: var(--bg-3);
    color: var(--text);
  }

  .pop-title {
    padding: 4px 6px 8px;
  }

  .swatches {
    display: flex;
    gap: 10px;
    padding: 2px 6px 6px;
  }

  .swatch {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    border: 2px solid transparent;
    background: var(--s);
    box-shadow: 0 0 0 1px var(--line-3), 0 4px 10px -2px color-mix(in oklab, var(--s) 60%, transparent);
    transition: transform 0.12s;
  }

  .swatch:hover {
    transform: scale(1.12);
  }

  .swatch.on {
    border-color: var(--bg-2);
    box-shadow: 0 0 0 2px var(--s), 0 4px 14px -2px color-mix(in oklab, var(--s) 70%, transparent);
  }

  main {
    min-height: 0;
    min-width: 0;
    padding: 18px 22px;
    overflow: hidden;
  }

  .mixer {
    display: flex;
    gap: 18px;
    height: 100%;
    min-height: 0;
  }

  .group {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-height: 0;
    flex: none;
  }

  /* inputs take the free width and scroll on their own; the outputs stay pinned right, like a console */
  .group.inputs {
    flex: 1;
    min-width: 0;
  }

  .group.inputs .row {
    overflow-x: auto;
    padding: 2px 4px 8px 2px;
    margin: 0 -4px 0 -2px;
  }

  .group h2 {
    padding-left: 4px;
  }

  .group .row {
    display: flex;
    gap: 12px;
    flex: 1;
    min-height: 0;
  }

  .group .row > :global(*) {
    max-height: 640px;
  }

  .divider {
    width: 1px;
    flex: none;
    background: linear-gradient(180deg, transparent, var(--line-2) 12%, var(--line-2) 88%, transparent);
  }

  .scroll {
    height: 100%;
    overflow-y: auto;
    padding-right: 6px;
  }

  .proc {
    display: grid;
    grid-template-columns: 210px minmax(0, 1fr);
    gap: 18px;
    align-items: start;
  }

  .picker {
    display: flex;
    flex-direction: column;
    gap: 6px;
    position: sticky;
    top: 0;
  }

  .picker button {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 9px 12px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--bg-2);
    color: var(--text-2);
    text-align: left;
    font-weight: 550;
    transition: all 0.12s;
  }

  .picker button:hover {
    border-color: var(--line-3);
    color: var(--text);
  }

  .picker button.active {
    color: var(--text);
    border-color: color-mix(in oklab, var(--accent) 55%, transparent);
    background: var(--accent-soft);
  }

  .picker .n {
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    display: grid;
    place-items: center;
    font: 700 11px var(--mono);
    background: var(--bg-0);
    color: var(--accent-hi);
  }

  .picker .t {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .stack {
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
  }

  .connect {
    display: flex;
    align-items: center;
    gap: 22px;
    max-width: 580px;
    margin: 12vh auto 0;
  }

  .connect h2 {
    font-size: 16px;
    font-weight: 650;
    color: var(--text);
    letter-spacing: 0;
    text-transform: none;
    margin-bottom: 4px;
  }

  .connect .err {
    margin-top: 10px;
    height: auto;
    padding: 4px 10px;
    white-space: normal;
  }

  code {
    font-family: var(--mono);
    background: var(--bg-0);
    padding: 1px 6px;
    border-radius: 5px;
    border: 1px solid var(--line);
  }

  .pulse {
    position: relative;
    flex: none;
    width: 44px;
    height: 44px;
  }

  .pulse i {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    border: 2px solid var(--accent);
    opacity: 0;
    animation: ripple 2.1s infinite var(--ease);
  }

  .pulse i:nth-child(2) {
    animation-delay: 0.7s;
  }

  .pulse i:nth-child(3) {
    animation-delay: 1.4s;
  }

  @keyframes ripple {
    0% {
      transform: scale(0.35);
      opacity: 0.9;
    }
    100% {
      transform: scale(1);
      opacity: 0;
    }
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 9px 22px;
    border-top: 1px solid var(--line);
    background: color-mix(in oklab, var(--bg-1) 70%, transparent);
    font-size: 12px;
  }

  .ver {
    font-size: 11px;
  }

  @media (max-width: 1180px) {
    .status-pill .state,
    .status-pill .sep {
      display: none;
    }

    .top {
      gap: 8px;
      padding-inline: 16px;
    }

    main {
      padding-inline: 16px;
    }
  }
</style>
