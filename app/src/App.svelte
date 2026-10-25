<script lang="ts">
  import { appColor, appLabel, CHANNELS, channelMeta } from './lib/channels.ts';
  import { dnd } from './lib/dnd.svelte.ts';
  import { engine } from './lib/engine.svelte.ts';
  import { ACCENTS, periodLabel, rateLabel, shortName, type AccentId } from './lib/theme.ts';
  import type { ChannelKey } from './lib/protocol.ts';
  import ChannelColumn from './components/ChannelColumn.svelte';
  import ChannelPage from './components/ChannelPage.svelte';
  import MasterColumn from './components/MasterColumn.svelte';
  import Recorder from './components/Recorder.svelte';
  import Hardware from './components/Hardware.svelte';
  import Setup from './components/Setup.svelte';
  import System from './components/System.svelte';
  import Icon, { type IconName } from './components/Icon.svelte';
  import Popover from './components/Popover.svelte';
  import CommandPalette, { type Command } from './components/CommandPalette.svelte';
  import Toasts from './components/Toasts.svelte';
  import { toast } from './lib/toast.svelte.ts';

  engine.start();

  type Tab = 'mixer' | ChannelKey | 'record' | 'hardware' | 'system';

  function remembered<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
    try {
      const v = localStorage.getItem(key) as T | null;
      return v && allowed.includes(v) ? v : fallback;
    } catch {
      return fallback;
    }
  }

  const accentIds = ACCENTS.map((a) => a.id);
  let accent = $state<AccentId>(remembered('audiora.accent', accentIds, 'azure'));
  let tab = $state<Tab>('mixer');

  $effect(() => {
    document.documentElement.dataset.accent = accent;
    try {
      localStorage.setItem('audiora.accent', accent);
    } catch {
      /* private mode: preferences just are not remembered */
    }
  });

  interface NavItem {
    id: Tab;
    label: string;
    icon: IconName;
    title: string;
    color?: string;
    /** Starts the second group of the rail. */
    divider?: boolean;
  }

  const NAV: NavItem[] = [
    { id: 'mixer', label: 'Mixer', icon: 'mixer', title: 'Mixer' },
    ...CHANNELS.map((c) => ({ id: c.key as Tab, label: c.name, icon: c.icon, title: c.name, color: c.color })),
    { id: 'record', label: 'Record', icon: 'record', title: 'Recorder', divider: true },
    { id: 'hardware', label: 'Interface', icon: 'interface', title: 'Audio interface' },
    { id: 'system', label: 'Setup', icon: 'system', title: 'Setup & health' },
  ];

  const channelList = $derived(engine.channels?.channels ?? []);
  const buses = $derived(engine.info?.buses ?? []);
  const currentChannel = $derived(channelList.find((c) => c.key === tab));

  // ---- command palette, shortcuts, error toasts -----------------------------------------
  let paletteOpen = $state(false);

  const commands = $derived.by<Command[]>(() => {
    const list: Command[] = NAV.map((n, i) => ({
      id: `go-${n.id}`,
      title: `Go to ${n.title}`,
      group: 'Navigate',
      icon: n.icon,
      hint: `Alt+${i + 1}`,
      run: () => (tab = n.id),
    }));
    if (engine.connected) {
      for (const c of CHANNELS) {
        const lists = engine.chPresets[c.key];
        for (const name of [...(lists?.favorites ?? [])]) {
          list.push({
            id: `cp-${c.key}-${name}`,
            title: `${c.name}: ${name}`,
            group: 'Channel presets',
            icon: 'star',
            run: () => void engine.loadChannelPreset(c.key, name).then(() => toast(`${c.name} → ${name}`, 'ok')).catch((e) => toast(String(e), 'error')),
          });
        }
        const mute = `strip/${CHANNELS.indexOf(c)}/mute`;
        list.push({
          id: `mute-${c.key}`,
          title: `${engine.get(mute) > 0.5 ? 'Unmute' : 'Mute'} ${c.name}`,
          group: 'Actions',
          icon: c.icon,
          run: () => engine.toggle(mute),
        });
      }
      for (const name of engine.presets) {
        list.push({
          id: `preset-${name}`,
          title: `Load scene “${name}”`,
          group: 'Scenes',
          icon: 'save',
          run: () => void engine.loadPreset(name).then(() => toast(`Loaded “${name}”`, 'ok')).catch((e) => toast(String(e), 'error')),
        });
      }
      list.push(
        engine.rec.recording
          ? {
              id: 'rec-stop',
              title: 'Stop recording',
              group: 'Actions',
              icon: 'stop',
              run: () => void engine.recStop().then((r) => toast(`Take saved · ${(r.seconds as number).toFixed(1)} s`, 'ok')).catch((e) => toast(String(e), 'error')),
            }
          : {
              id: 'rec-start',
              title: 'Start recording the master mix',
              group: 'Actions',
              icon: 'record',
              run: () => void engine.recStart(['bus/0']).then(() => toast('Recording started', 'ok')).catch((e) => toast(String(e), 'error')),
            },
        {
          id: 'mute-master',
          title: engine.get('bus/0/mute') > 0.5 ? 'Unmute master output' : 'Mute master output',
          group: 'Actions',
          icon: 'speaker',
          run: () => engine.toggle('bus/0/mute'),
        },
        {
          id: 'lufs-reset',
          title: 'Restart loudness measurement',
          group: 'Actions',
          icon: 'bolt',
          run: () => engine.resetLoudness(),
        },
      );
    }
    for (const a of ACCENTS) {
      list.push({ id: `accent-${a.id}`, title: `Accent colour: ${a.label}`, group: 'Appearance', icon: 'palette', run: () => (accent = a.id) });
    }
    return list;
  });

  function onwindowkey(e: KeyboardEvent) {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      paletteOpen = !paletteOpen;
      return;
    }
    if (e.altKey && /^[1-9]$/.test(e.key)) {
      const target = NAV[Number(e.key) - 1];
      if (target) {
        e.preventDefault();
        tab = target.id;
      }
    }
  }

  let shownError = '';
  $effect(() => {
    const err = engine.lastError;
    if (err && err !== shownError) toast(err.slice(0, 200), 'error');
    shownError = err;
  });

  const current = $derived(NAV.find((n) => n.id === tab)!);
  const master = $derived(buses[0]);
  const s = $derived(engine.stats);
  const glitches = $derived(s.underruns + s.drift_drops + s.capture_drops + s.device_errors);
  const routedCount = $derived(engine.appGroups.filter((a) => a.route).length);
  const ready = $derived(channelList.filter((c) => c.kind === 'apps' && c.online).length);
  const subtitle = $derived(
    tab === 'mixer'
      ? `${ready} of 4 app channels ready · ${routedCount} app${routedCount === 1 ? '' : 's'} routed`
      : currentChannel
        ? currentChannel.kind === 'mic'
          ? currentChannel.online
            ? shortName(currentChannel.device_name ?? 'Microphone')
            : 'No microphone found'
          : currentChannel.online
            ? `${engine.appGroups.filter((a) => a.route === currentChannel.key).length} apps · ${currentChannel.mode === 'direct' ? 'direct capture' : shortName(currentChannel.device_name ?? '')}`
            : 'Not set up'
        : tab === 'record'
          ? '32-bit float WAV, multitrack'
          : tab === 'hardware'
            ? 'Preamps, phantom power, monitoring'
            : 'Devices, channels and audio health',
  );
</script>

<svelte:window onkeydown={onwindowkey} />

<div class="app">
  <nav class="rail" aria-label="Sections">
    <div class="logo" aria-label="Audiora">
      <svg viewBox="0 0 24 24" width="26" height="26" aria-hidden="true"><path d="M3 14v-4M7 18V6M11 21V3M15 17V7M19 14v-4" stroke="#fff" stroke-width="2.4" stroke-linecap="round" fill="none" /></svg>
    </div>
    <div class="items">
      {#each NAV as n (n.id)}
        {#if n.divider}<hr />{/if}
        <button class="item" class:active={tab === n.id} style:--c={n.color ?? 'var(--accent)'} aria-current={tab === n.id ? 'page' : undefined} onclick={() => (tab = n.id)}>
          <span class="ico"><Icon name={n.icon} size={19} />{#if n.id === 'record' && engine.rec.recording}<i class="recdot" aria-label="recording"></i>{/if}</span>
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
          <span class="state">Running</span>
          <span class="sep"></span>
          <span class="num">{rateLabel(master.sample_rate)} · {periodLabel(master.sample_rate, master.period_frames)}</span>
          {#if master.exclusive}<span class="chip accent">EXCLUSIVE</span>{:else if master.low_latency}<span class="chip accent">LOW LATENCY</span>{/if}
        </div>
      {/if}


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
      {:else if !engine.channelMode && tab !== 'record' && tab !== 'hardware' && tab !== 'system'}
        <div class="connect panel pad" role="status">
          <span class="warn-ico"><Icon name="alert" size={26} /></span>
          <div>
            <h2>This engine runs without channels</h2>
            <p class="muted">It was started with explicit <code>--in</code> / <code>--no-input</code> device flags. Restart <code>audiora-svc</code> without them to get Game, Chat, Media, Aux and Mic.</p>
          </div>
        </div>
      {:else if tab === 'mixer'}
        <div class="mixer">
          <MasterColumn />
          <div class="divider"></div>
          <div class="channels">
            {#each channelList as c (c.key)}
              <ChannelColumn ch={c} onopen={() => (tab = c.key)} onsetup={() => (tab = 'system')} />
            {/each}
          </div>
        </div>
      {:else if currentChannel}
        {#key currentChannel.key}
          <div class="scroll"><ChannelPage ch={currentChannel} onsetup={() => (tab = 'system')} /></div>
        {/key}
      {:else if tab === 'record'}
        <div class="scroll"><Recorder /></div>
      {:else if tab === 'hardware'}
        <div class="scroll"><Hardware /></div>
      {:else}
        <div class="scroll stack">
          {#if engine.channelMode}<Setup />{/if}
          <System />
        </div>
      {/if}
    </main>

    <CommandPalette open={paletteOpen} {commands} onclose={() => (paletteOpen = false)} />
    <Toasts />

    {#if dnd.drag}
      <div class="drag-ghost" style:left="{dnd.drag.x}px" style:top="{dnd.drag.y}px" style:--c={appColor(dnd.drag.name)} aria-hidden="true">
        <span class="av">{appLabel(dnd.drag.name).charAt(0)}</span>
        <span>{appLabel(dnd.drag.name)}</span>
        {#if dnd.over && dnd.over !== 'none'}<b>→ {channelMeta(dnd.over)?.name}</b>{/if}
      </div>
    {/if}

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
    gap: 3px;
    width: 100%;
    padding: 0 10px;
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    scrollbar-width: none;
  }

  .items hr {
    width: 28px;
    margin: 6px auto;
    border: 0;
    border-top: 1px solid var(--line-2);
  }

  .item {
    display: flex;
    flex-direction: column;
    align-items: center;
    flex: none;
    gap: 3px;
    padding: 7px 0 6px;
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
    color: var(--c);
    background: color-mix(in oklab, var(--c) 15%, transparent);
  }

  .item.active::before {
    content: '';
    position: absolute;
    left: -10px;
    top: 14px;
    bottom: 14px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--c);
    box-shadow: 0 0 12px var(--c);
  }

  .ico {
    position: relative;
    display: grid;
  }

  .lbl {
    font-size: 10px;
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


  /* every view fades and rises in when it is opened */
  .mixer,
  .scroll,
  .connect {
    animation: view-in 0.24s var(--ease) both;
  }

  @keyframes view-in {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
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
    gap: 16px;
    height: 100%;
    min-height: 0;
  }

  .channels {
    flex: 1;
    min-width: 0;
    display: flex;
    gap: 12px;
    min-height: 0;
    overflow-x: auto;
    padding: 2px 4px 8px 2px;
    margin: 0 -4px 0 -2px;
  }

  .channels > :global(.col) {
    max-height: 760px;
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

  .stack {
    display: flex;
    flex-direction: column;
    gap: 16px;
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

  .warn-ico {
    flex: none;
    width: 52px;
    height: 52px;
    border-radius: 16px;
    display: grid;
    place-items: center;
    color: var(--warn);
    background: color-mix(in oklab, var(--warn) 12%, transparent);
  }

  /* the app being dragged follows the pointer */
  .drag-ghost {
    position: fixed;
    z-index: 200;
    display: flex;
    align-items: center;
    gap: 9px;
    padding: 7px 14px 7px 8px;
    border-radius: 999px;
    background: linear-gradient(180deg, var(--bg-4), var(--bg-3));
    border: 1px solid color-mix(in oklab, var(--c) 55%, transparent);
    box-shadow: var(--shadow-pop), 0 0 28px -6px var(--c);
    font-weight: 650;
    pointer-events: none;
    transform: translate(14px, 10px) rotate(-2deg);
  }

  .drag-ghost .av {
    width: 26px;
    height: 26px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    color: #0b0e13;
    font-weight: 800;
    background: var(--c);
  }

  .drag-ghost b {
    color: var(--accent-hi);
    font-weight: 650;
  }

  :global(body.dragging-app),
  :global(body.dragging-app *) {
    cursor: grabbing !important;
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
