<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { linToDb } from '../lib/dsp.ts';
  import { shortName } from '../lib/theme.ts';
  import type { AppSessionEntry } from '../lib/protocol.ts';
  import Icon from './Icon.svelte';
  import LedMeter from './LedMeter.svelte';

  let loading = $state(true);

  $effect(() => {
    let alive = true;
    const tick = async () => {
      try {
        await engine.refreshApps();
      } catch {
        /* the next tick retries */
      }
      loading = false;
    };
    void tick();
    const timer = setInterval(() => alive && void tick(), 500);
    return () => {
      alive = false;
      clearInterval(timer);
    };
  });

  // Windows own audio host appears as a session; it is not something to mix.
  const shown = $derived(engine.apps.filter((a) => a.name !== 'audiodg'));
  const active = $derived(shown.filter((a) => a.active));
  // Many apps (Steam, Windows sounds) hold an idle session on every playback device, virtual ones included.
  const idle = $derived(shown.filter((a) => !a.active && !a.system_sounds));
  const label = (a: AppSessionEntry) => (a.name === 'audiora-svc' ? 'Audiora engine' : a.name);
  const key = (a: AppSessionEntry) => `${a.device_id}|${a.instance_id}`;

  const HUES = [210, 160, 330, 265, 190, 45, 12, 95];
  function hue(name: string) {
    let h = 0;
    for (const ch of name) h = (h * 31 + ch.charCodeAt(0)) >>> 0;
    return HUES[h % HUES.length];
  }
</script>

{#snippet row(a: AppSessionEntry)}
  <div class="app" class:idle={!a.active} class:muted={a.muted} style:--h={hue(a.name)}>
    <span class="avatar" aria-hidden="true">{label(a).slice(0, 1).toUpperCase()}</span>
    <div class="who">
      <strong title={a.name}>{label(a)}</strong>
      <span class="dev" title={a.device_name}><Icon name="speaker" size={11} /> {shortName(a.device_name)}</span>
    </div>
    <div class="lvl">
      <LedMeter orientation="horizontal" scale={false} thickness={6} levels={[a.muted ? -120 : linToDb(a.peak)]} />
    </div>
    <input
      type="range"
      min="0"
      max="100"
      step="1"
      style:--fill="{Math.round(a.volume * 100)}%"
      value={Math.round(a.volume * 100)}
      aria-label="{label(a)} volume"
      oninput={(e) => engine.setApp(a, { volume: +e.currentTarget.value / 100 })}
    />
    <output class="num">{Math.round(a.volume * 100)}</output>
    <button
      class="mute"
      class:on={a.muted}
      aria-pressed={a.muted}
      aria-label="Mute {label(a)}"
      title={a.muted ? 'Unmute' : 'Mute'}
      onclick={() => engine.setApp(a, { mute: !a.muted })}
    >
      M
    </button>
  </div>
{/snippet}

<div class="apps">
  <p class="muted intro">
    Per-application volume, exactly like the Windows volume mixer. Sending an application to a separate
    Audiora channel needs the Audiora virtual driver, which is not installed yet.
  </p>

  {#if loading}
    <p class="muted">Looking for applications…</p>
  {:else}
    <section class="panel list">
      <header><h2 class="eyebrow">Playing now</h2><span class="chip">{active.length}</span></header>
      {#each active as a (key(a))}{@render row(a)}{:else}
        <p class="empty muted">Nothing is playing. Start a game, a browser tab or a music player and it shows up here.</p>
      {/each}
    </section>

    {#if idle.length}
      <details class="panel list idle-group">
        <summary><h2 class="eyebrow">Open but silent</h2><span class="chip">{idle.length}</span></summary>
        {#each idle as a (key(a))}{@render row(a)}{/each}
      </details>
    {/if}
  {/if}
</div>

<style>
  .apps {
    display: flex;
    flex-direction: column;
    gap: 14px;
    max-width: 980px;
  }

  .intro {
    font-size: 12.5px;
    max-width: 74ch;
  }

  .list {
    padding: 8px 8px 6px;
  }

  header,
  summary {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 8px 12px 10px;
  }

  summary {
    cursor: pointer;
    list-style: none;
  }

  summary::-webkit-details-marker {
    display: none;
  }

  summary::before {
    content: '▸';
    color: var(--faint);
    font-size: 11px;
    transition: transform 0.15s;
  }

  details[open] summary::before {
    transform: rotate(90deg);
  }

  .app {
    display: grid;
    grid-template-columns: 38px minmax(150px, 1.2fr) 130px minmax(160px, 1.5fr) 34px 30px;
    align-items: center;
    gap: 14px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    transition: background 0.12s;
  }

  .app:hover {
    background: var(--bg-3);
  }

  .app.idle {
    opacity: 0.6;
  }

  .avatar {
    width: 36px;
    height: 36px;
    border-radius: 11px;
    display: grid;
    place-items: center;
    font-weight: 700;
    font-size: 15px;
    color: hsl(var(--h) 90% 78%);
    background: linear-gradient(135deg, hsl(var(--h) 55% 24%), hsl(var(--h) 55% 15%));
    border: 1px solid hsl(var(--h) 55% 35% / 0.6);
  }

  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .who strong {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dev {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    color: var(--faint);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .lvl {
    min-width: 0;
  }

  output {
    text-align: right;
    font-size: 12px;
    color: var(--text-2);
  }

  .mute {
    height: 26px;
    border-radius: 7px;
    border: 1px solid var(--line-2);
    background: var(--bg-2);
    font: 700 11px var(--font);
    color: var(--muted);
  }

  .mute:hover {
    color: var(--text);
  }

  .mute.on {
    background: var(--bad);
    border-color: transparent;
    color: #230408;
    box-shadow: 0 0 12px -2px var(--bad);
  }

  .app.muted .avatar {
    filter: grayscale(0.8);
  }

  .empty {
    padding: 22px 14px 26px;
    text-align: center;
    font-size: 12.5px;
  }
</style>
