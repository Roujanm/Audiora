<script lang="ts">
  import { appColor, appLabel, CHANNELS, channelMeta } from './lib/channels.ts';
  import { dnd } from './lib/dnd.svelte.ts';
  import { engine } from './lib/engine.svelte.ts';
  import { ACCENTS, periodLabel, rateLabel, shortName, type AccentId } from './lib/theme.ts';
  import type { ChannelKey } from './lib/protocol.ts';
  import ChannelColumn from './components/ChannelColumn.svelte';
  import ChannelPage from './components/ChannelPage.svelte';
  import AppsDock from './components/AppsDock.svelte';
  import MasterStage from './components/MasterStage.svelte';
  import VoiceStudio from './components/VoiceStudio.svelte';
  import Studio from './components/Studio.svelte';
  import Hardware from './components/Hardware.svelte';
  import Setup from './components/Setup.svelte';
  import System from './components/System.svelte';
  import Icon, { type IconName } from './components/Icon.svelte';
  import Popover from './components/Popover.svelte';
  import CommandPalette, { type Command } from './components/CommandPalette.svelte';
  import Toasts from './components/Toasts.svelte';
  import Logo from './components/Logo.svelte';
  import Splash from './components/Splash.svelte';
  import UpdateBanner from './components/UpdateBanner.svelte';
  import { desktop } from './lib/desktop.svelte.ts';
  import { toast } from './lib/toast.svelte.ts';

  engine.start();
  void desktop.start();

  type Tab = 'mixer' | ChannelKey | 'studio' | 'hardware' | 'system';

  function remembered<T extends string>(key: string, allowed: readonly T[], fallback: T): T {
    try {
      const v = localStorage.getItem(key) as T | null;
      return v && allowed.includes(v) ? v : fallback;
    } catch {
      return fallback;
    }
  }

  const accentIds = ACCENTS.map((a) => a.id);
  // the black-and-white Audiora identity becomes the look once, for installs that kept the old default
  try {
    if (!localStorage.getItem('audiora.brand-v1')) {
      localStorage.setItem('audiora.brand-v1', '1');
      localStorage.setItem('audiora.accent', 'mono');
    }
  } catch {
    /* storage unavailable: defaults apply */
  }
  let accent = $state<AccentId>(remembered('audiora.accent', accentIds, 'mono'));
  let splash = $state(true);
  const TABS: Tab[] = ['mixer', 'game', 'chat', 'media', 'aux', 'mic', 'studio', 'hardware', 'system'];
  let tab = $state<Tab>(remembered('audiora.tab', TABS, 'mixer'));

  $effect(() => {
    document.documentElement.dataset.accent = accent;
    try {
      localStorage.setItem('audiora.accent', accent);
      localStorage.setItem('audiora.tab', tab);
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

  /** Left rail: the big areas of the product. */
  const RAIL: NavItem[] = [
    { id: 'mixer', label: 'Mixer', icon: 'mixer', title: 'Mixer' },
    { id: 'mic', label: 'Voice', icon: 'mic', title: 'Voice' },
    { id: 'studio', label: 'Studio', icon: 'record', title: 'Studio' },
    { id: 'hardware', label: 'Interface', icon: 'interface', title: 'Audio interface' },
    { id: 'system', label: 'Setup', icon: 'system', title: 'Setup & health' },
  ];

  /** Header tabs of the mixer area: the overview and one page per channel. */
  const CHANNEL_TABS: NavItem[] = [
    { id: 'mixer', label: 'Overview', icon: 'mixer', title: 'Mixer' },
    // the microphone has its own section (Voice); these are the application channels
    ...CHANNELS.filter((c) => c.key !== 'mic').map((c) => ({ id: c.key as Tab, label: c.name, icon: c.icon, title: c.name, color: c.color })),
  ];

  const NAV: NavItem[] = [...CHANNEL_TABS, ...RAIL.slice(1)];
  const inMixer = $derived(tab === 'mixer' || CHANNELS.some((c) => c.key === tab && c.key !== 'mic'));

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
  // the aurora breathes with the master output (0 = silence, 1 = loud)
  const auroraLevel = $derived(
    Math.max(0, Math.min(1, (Math.max(engine.peakDb('bus/0/peak_l'), engine.peakDb('bus/0/peak_r')) + 50) / 44)).toFixed(2),
  );
  const master = $derived(buses[0]);
  const s = $derived(engine.stats);
  const glitches = $derived(s.underruns + s.drift_drops + s.capture_drops + s.device_errors);
  const glitchTitle = $derived(
    engine.glitchLog.length
      ? ['Recent glitches:', ...engine.glitchLog.slice(0, 8).map((g) => `${new Date(g.t * 1000).toLocaleTimeString()}  ${g.kind} · ${g.place}`)].join(
          String.fromCharCode(10),
        )
      : 'Underruns, drift drops, dropped blocks and device errors since start',
  );
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
        : tab === 'studio'
          ? 'Multitrack recording of every channel'
          : tab === 'hardware'
            ? 'Preamps, phantom power, monitoring'
            : 'Devices, channels and audio health',
  );
</script>

<svelte:window onkeydown={onwindowkey} />

{#if splash}<Splash ondone={() => (splash = false)} />{/if}

<div class="app" style:--lvl={auroraLevel}>
  <div class="aurora" aria-hidden="true"><i></i><i></i><i></i></div>

  <div class="shell">
    <header class="top">
      <div class="brand" aria-label="Audiora">
        <span class="logo"><Logo variant="glyph" size={22} /></span>
        <span class="word">Audiora</span>
      </div>

      <nav class="primary" aria-label="Sections">
        {#each RAIL as n (n.id)}
          {@const active = n.id === 'mixer' ? inMixer : tab === n.id}
          <button class="item" class:active aria-current={active ? 'page' : undefined} onclick={() => (tab = n.id)}>
            <Icon name={n.icon} size={16} />
            <span>{n.label}</span>
            {#if n.id === 'studio' && engine.rec.recording}<i class="recdot" aria-label="recording"></i>{/if}
          </button>
        {/each}
      </nav>

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

    {#if inMixer && engine.connected}
      <nav class="tabs" aria-label="Channels">
        {#each CHANNEL_TABS as t (t.id)}
          <button class="tab" class:active={tab === t.id} style:--c={t.color ?? 'var(--accent)'} aria-current={tab === t.id ? 'page' : undefined} onclick={() => (tab = t.id)}>
            <Icon name={t.icon} size={15} />
            <span>{t.label}</span>
          </button>
        {/each}
      </nav>
    {:else if !inMixer && tab !== 'mic'}
      <div class="title">
        <h1>{current.title}</h1>
        <p class="muted">{subtitle}</p>
      </div>
    {/if}

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
      {:else if !engine.channelMode && tab !== 'studio' && tab !== 'hardware' && tab !== 'system'}
        <div class="connect panel pad" role="status">
          <span class="warn-ico"><Icon name="alert" size={26} /></span>
          <div>
            <h2>This engine runs without channels</h2>
            <p class="muted">It was started with explicit <code>--in</code> / <code>--no-input</code> device flags. Restart <code>audiora-svc</code> without them to get Game, Chat, Media, Aux and Mic.</p>
          </div>
        </div>
      {:else if tab === 'mixer'}
        <div class="overview">
          <MasterStage />
          <div class="mixer">
          <div class="side"><AppsDock /></div>
          <div class="channels">
            {#each channelList as c (c.key)}
              <ChannelColumn ch={c} onopen={() => (tab = c.key)} onsetup={() => (tab = 'system')} />
            {/each}
          </div>
          </div>
        </div>
      {:else if currentChannel?.kind === 'mic'}
        <div class="scroll"><VoiceStudio ch={currentChannel} /></div>
      {:else if currentChannel}
        {#key currentChannel.key}
          <div class="scroll"><ChannelPage ch={currentChannel} onsetup={() => (tab = 'system')} /></div>
        {/key}
      {:else if tab === 'studio'}
        <div class="fill"><Studio /></div>
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
    <UpdateBanner />

    {#if dnd.drag}
      <div class="drag-ghost" style:left="{dnd.drag.x}px" style:top="{dnd.drag.y}px" style:--c={appColor(dnd.drag.name)} aria-hidden="true">
        <span class="av">{appLabel(dnd.drag.name).charAt(0)}</span>
        <span>{appLabel(dnd.drag.name)}</span>
        {#if dnd.over && dnd.over !== 'none'}<b>→ {channelMeta(dnd.over)?.name}</b>{/if}
      </div>
    {/if}

    <footer class="foot">
      {#if engine.connected}
        <span class="chip" class:warn={glitches > 6} title={glitchTitle}>
          {glitches === 0 ? 'No audio glitches' : `${glitches} glitch events`}
        </span>
        {#if engine.rec.recording}<span class="chip bad">● Recording</span>{/if}
        {#if engine.hwInfo?.simulated}<span class="chip warn">Interface simulator</span>{/if}
        {#if engine.lastError}<span class="chip bad" title={engine.lastError}>{engine.lastError.slice(0, 90)}</span>{/if}
      {:else}
        <span class="chip bad">Offline</span>
      {/if}
      <span class="spacer"></span>
      <span class="muted ver">Audiora {desktop.version || '0.1'} · {engine.license?.state === 'valid' ? `${engine.license.edition} licence` : 'Play edition'}</span>
    </footer>
  </div>
</div>

<style>
  .app {
    position: relative;
    height: 100%;
    isolation: isolate;
  }

  /* ----- aurora: three slow light fields behind everything, brighter while sound plays */
  .aurora {
    position: fixed;
    inset: -20%;
    z-index: -1;
    pointer-events: none;
    filter: blur(70px) saturate(130%);
    opacity: calc(0.45 + var(--lvl, 0) * 0.45);
    transition: opacity 0.35s linear;
  }

  .aurora i {
    position: absolute;
    width: 46vw;
    height: 46vw;
    border-radius: 50%;
    mix-blend-mode: screen;
  }

  .aurora i:nth-child(1) {
    left: 8%;
    top: 4%;
    background: radial-gradient(circle, color-mix(in oklab, var(--accent) 30%, #3a6bff 30%), transparent 65%);
    animation: drift-a 26s ease-in-out infinite alternate;
  }

  .aurora i:nth-child(2) {
    right: 4%;
    top: 18%;
    background: radial-gradient(circle, color-mix(in oklab, #2ed3e6 40%, transparent), transparent 65%);
    animation: drift-b 32s ease-in-out infinite alternate;
  }

  .aurora i:nth-child(3) {
    left: 32%;
    bottom: -6%;
    background: radial-gradient(circle, color-mix(in oklab, #a58bff 38%, transparent), transparent 65%);
    animation: drift-c 38s ease-in-out infinite alternate;
  }

  @keyframes drift-a {
    to {
      transform: translate(14vw, 8vh) scale(1.15);
    }
  }

  @keyframes drift-b {
    to {
      transform: translate(-16vw, 12vh) scale(0.9);
    }
  }

  @keyframes drift-c {
    to {
      transform: translate(10vw, -14vh) scale(1.2);
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .aurora i {
      animation: none;
    }
  }

  .recdot {
    position: absolute;
    top: 6px;
    right: 6px;
    width: 7px;
    height: 7px;
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

  /* ----- shell */
  .shell {
    display: grid;
    grid-template-rows: auto auto 1fr auto;
    height: 100%;
    min-width: 0;
    min-height: 0;
  }

  .top {
    position: relative;
    z-index: 20; /* popovers open over the page below */
    display: flex;
    align-items: center;
    gap: 14px;
    margin: 14px 18px 0;
    padding: 8px 10px 8px 14px;
    border-radius: 18px;
    background: var(--glass);
    border: 1px solid var(--glass-line);
    backdrop-filter: blur(22px) saturate(150%);
    box-shadow: var(--shadow-2);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-right: 6px;
  }

  .logo {
    width: 34px;
    height: 34px;
    display: grid;
    place-items: center;
    border-radius: 10px;
    color: #fff;
    background: #000;
    box-shadow: 0 0 0 1px rgba(255, 255, 255, 0.14), 0 6px 16px -6px rgba(0, 0, 0, 0.9);
  }

  .word {
    font-size: 15px;
    font-weight: 750;
    letter-spacing: 0.02em;
  }

  .primary {
    display: flex;
    gap: 2px;
    padding: 3px;
    border-radius: 13px;
    background: rgba(0, 0, 0, 0.25);
    border: 1px solid var(--glass-line);
  }

  .item {
    position: relative;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 14px;
    border: 0;
    border-radius: 10px;
    background: none;
    color: var(--muted);
    font-weight: 600;
    font-size: 13px;
    transition: color 0.15s, background 0.15s;
  }

  .item:hover {
    color: var(--text);
  }

  .item.active {
    color: var(--on-accent);
    background: linear-gradient(180deg, var(--accent-hi), var(--accent));
    box-shadow: 0 6px 18px -8px var(--accent-glow);
  }

  .title {
    margin: 16px 26px 0;
    min-width: 0;
  }

  .title p {
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    font-size: 12px;
  }

  .title h1 {
    font-size: 24px;
    font-weight: 750;
    letter-spacing: -0.02em;
    line-height: 1.15;
  }

  .spacer {
    flex: 1;
  }

  .tabs {
    display: flex;
    gap: 6px;
    margin: 14px 18px 0;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
  }

  .tab {
    flex: none;
    display: flex;
    align-items: center;
    gap: 8px;
    height: 34px;
    padding: 0 15px 0 12px;
    border-radius: 999px;
    border: 1px solid var(--glass-line);
    background: var(--glass);
    backdrop-filter: blur(14px);
    color: var(--text-2);
    font-weight: 600;
    font-size: 13px;
    transition: color 0.15s, background 0.15s, border-color 0.15s;
  }

  .tab :global(svg) {
    color: var(--c);
  }

  .tab:hover {
    color: var(--text);
    border-color: color-mix(in oklab, var(--c) 40%, var(--glass-line));
  }

  .tab.active {
    color: var(--text);
    border-color: color-mix(in oklab, var(--c) 70%, transparent);
    background: linear-gradient(180deg, color-mix(in oklab, var(--c) 30%, transparent), color-mix(in oklab, var(--c) 12%, transparent));
    box-shadow: 0 0 22px -6px var(--c);
  }

  .overview {
    display: flex;
    flex-direction: column;
    gap: 14px;
    height: 100%;
    min-height: 0;
  }

  .overview .mixer {
    flex: 1;
  }

  .side {
    width: 248px;
    flex: none;
    display: flex;
    flex-direction: column;
    min-height: 0;
  }

  .side > :global(*) {
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
    padding: 14px 18px 10px;
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

  .scroll {
    height: 100%;
    overflow-y: auto;
    padding-right: 6px;
  }

  .fill {
    height: 100%;
    min-height: 0;
    animation: view-in 0.24s var(--ease) both;
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
    padding: 6px 26px 12px;
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
