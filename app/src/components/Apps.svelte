<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { linToDb } from '../lib/dsp.ts';
  import type { AppSessionEntry } from '../lib/protocol.ts';

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

  // Windows' own audio host shows up as a session; it is not something to mix.
  const shown = $derived(engine.apps.filter((a) => a.name !== 'audiodg'));
  const active = $derived(shown.filter((a) => a.active));
  // Many apps (Steam, Windows sounds) hold an idle session on every playback device, virtual ones included.
  const idle = $derived(shown.filter((a) => !a.active && !a.system_sounds));
  const label = (a: AppSessionEntry) => (a.name === 'audiora-svc' ? 'Audiora engine' : a.name);

  const level = (peak: number) => Math.min(Math.max((linToDb(peak) + 60) / 60, 0), 1);
  const key = (a: AppSessionEntry) => `${a.device_id}|${a.instance_id}`;
</script>

{#snippet row(a: AppSessionEntry)}
  <div class="app" class:idle={!a.active} class:muted={a.muted}>
    <div class="who">
      <strong title={a.name}>{label(a)}</strong>
      <span class="muted dev" title={a.device_name}>{a.device_name}</span>
    </div>
    <div class="meter" role="img" aria-label="{a.name} output level">
      <div class="fill" style:width="{a.muted ? 0 : level(a.peak) * 100}%"></div>
    </div>
    <input
      type="range"
      min="0"
      max="100"
      step="1"
      value={Math.round(a.volume * 100)}
      aria-label="{a.name} volume"
      oninput={(e) => engine.setApp(a, { volume: +e.currentTarget.value / 100 })}
    />
    <output class="num">{Math.round(a.volume * 100)}%</output>
    <button
      class="mute"
      class:on={a.muted}
      aria-pressed={a.muted}
      aria-label="Mute {a.name}"
      onclick={() => engine.setApp(a, { mute: !a.muted })}
    >
      M
    </button>
  </div>
{/snippet}

<div class="apps">
  <p class="muted intro">
    Application volumes exactly as in the Windows volume mixer. Moving an application to a different
    Audiora channel needs the Audiora virtual driver, which is not installed yet.
  </p>

  {#if loading}
    <p class="muted">Looking for applications…</p>
  {:else}
    {#if active.length}
      <section class="card">
        <h2>Playing now</h2>
        {#each active as a (key(a))}{@render row(a)}{/each}
      </section>
    {/if}
    {#if active.length === 0}
      <div class="card"><h2>Nothing is playing</h2><p class="muted">Start a game, browser tab or music player and it will show up here.</p></div>
    {/if}
    {#if idle.length}
      <details class="card idle-group">
        <summary>Open but silent ({idle.length})</summary>
        {#each idle as a (key(a))}{@render row(a)}{/each}
      </details>
    {/if}
  {/if}
</div>

<style>
  .apps {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .intro {
    margin: 0;
    font-size: 12px;
    max-width: 78ch;
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .app {
    display: grid;
    grid-template-columns: minmax(150px, 1.1fr) 110px minmax(160px, 1.6fr) 44px 30px;
    align-items: center;
    gap: 12px;
    padding: 8px 4px;
    border-top: 1px solid var(--line);
  }

  .app:first-of-type {
    border-top: 0;
  }

  .app.idle {
    opacity: 0.6;
  }

  summary {
    cursor: pointer;
    color: var(--muted);
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    padding: 2px 0 8px;
  }

  .who {
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  .who strong,
  .dev {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dev {
    font-size: 11px;
  }

  .meter {
    height: 8px;
    border-radius: 4px;
    background: var(--bg);
    border: 1px solid var(--line);
    overflow: hidden;
  }

  .fill {
    height: 100%;
    background: linear-gradient(90deg, var(--ok), #c9e060 70%, var(--warn) 88%, var(--bad));
    transition: width 0.12s linear;
  }

  input[type='range'] {
    width: 100%;
    margin: 0;
  }

  output {
    text-align: right;
    font-size: 12px;
  }

  .mute {
    height: 24px;
    border-radius: 4px;
    border: 1px solid var(--line);
    background: var(--panel-2);
    font-size: 11px;
    font-weight: 700;
    color: var(--muted);
  }

  .mute.on {
    background: var(--bad);
    border-color: var(--bad);
    color: #1b0508;
  }
</style>
