<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';

  const MAX_SOURCES = 8;

  const strips = $derived(engine.info?.strips ?? []);
  const buses = $derived(engine.info?.buses ?? []);
  let chosen = $state<string[]>(['bus/0']);
  let busy = $state(false);
  let message = $state('');
  let lastTake = $state('');

  const rec = $derived(engine.rec);
  const channels = $derived(chosen.length * 2);

  function toggle(src: string) {
    chosen = chosen.includes(src) ? chosen.filter((s) => s !== src) : [...chosen, src].slice(0, MAX_SOURCES);
  }

  async function start() {
    busy = true;
    message = '';
    try {
      await engine.recStart(chosen);
    } catch (e) {
      message = String(e instanceof Error ? e.message : e);
    }
    busy = false;
  }

  async function stop() {
    busy = true;
    try {
      const r = await engine.recStop();
      lastTake = `${r.path as string}  ·  ${(r.seconds as number).toFixed(1)} s${(r.dropped_frames as number) > 0 ? `  ·  ${r.dropped_frames} dropped frames` : ''}`;
    } catch (e) {
      message = String(e instanceof Error ? e.message : e);
    }
    busy = false;
  }

  function clock(sec: number) {
    const m = Math.floor(sec / 60);
    const s = Math.floor(sec % 60);
    const t = Math.floor((sec % 1) * 10);
    return `${String(m).padStart(2, '0')}:${String(s).padStart(2, '0')}.${t}`;
  }
</script>

<div class="rec">
  <div class="transport">
    <div class="clock num" class:live={rec.recording} aria-live="off">{clock(rec.seconds ?? 0)}</div>
    {#if rec.recording}
      <button class="btn stop" onclick={stop} disabled={busy}>■ Stop</button>
    {:else}
      <button class="btn recbtn" onclick={start} disabled={busy || chosen.length === 0}>● Record</button>
    {/if}
    <div class="info">
      {#if rec.recording}
        <span class="chip bad">● REC</span>
        <span class="muted">{rec.sources?.length} source{rec.sources?.length === 1 ? '' : 's'} → {(rec.sources?.length ?? 0) * 2} ch · 32-bit float</span>
        {#if (rec.dropped_frames ?? 0) > 0}<span class="chip bad">{rec.dropped_frames} dropped frames</span>{/if}
        {#if rec.error}<span class="chip bad">{rec.error}</span>{/if}
      {:else}
        <span class="muted">{channels} channel{channels === 1 ? '' : 's'} · 32-bit float WAV</span>
      {/if}
    </div>
  </div>

  {#if message}<p class="chip bad" role="alert">{message}</p>{/if}

  <div class="sources" class:locked={rec.recording}>
    <h2>Tracks</h2>
    <p class="muted hint">Every source is one stereo track. Inputs are recorded raw (before gain and EQ); buses are what you hear.</p>
    <div class="grid">
      {#each strips as s, i (`s${i}`)}
        {@const id = `strip/${i}`}
        <label class="src" class:on={chosen.includes(id)}>
          <input type="checkbox" checked={chosen.includes(id)} disabled={rec.recording} onchange={() => toggle(id)} />
          <span class="t">{s.kind === 'input' ? `Input ${i + 1}` : 'Tone'}</span>
          <span class="muted n">{s.name}</span>
        </label>
      {/each}
      {#each buses as b, i (`b${i}`)}
        {@const id = `bus/${i}`}
        <label class="src bus" class:on={chosen.includes(id)}>
          <input type="checkbox" checked={chosen.includes(id)} disabled={rec.recording} onchange={() => toggle(id)} />
          <span class="t">{i === 0 ? 'Master mix' : `Bus ${i + 1}`}</span>
          <span class="muted n">{b.name}</span>
        </label>
      {/each}
    </div>
  </div>

  <div class="where">
    <span class="muted">Folder</span>
    <code class="num">{engine.info?.recordings_dir}</code>
  </div>
  {#if rec.recording && rec.path}
    <div class="where"><span class="muted">Recording to</span><code class="num">{rec.path}</code></div>
  {:else if lastTake}
    <div class="where"><span class="muted">Last take</span><code class="num">{lastTake}</code></div>
  {/if}
</div>

<style>
  .rec {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .transport {
    display: flex;
    align-items: center;
    gap: 14px;
    flex-wrap: wrap;
  }

  .clock {
    font-size: 34px;
    letter-spacing: 0.02em;
    padding: 4px 14px;
    background: var(--bg);
    border: 1px solid var(--line);
    border-radius: var(--radius);
    color: var(--muted);
  }

  .clock.live {
    color: var(--rec);
    border-color: rgba(255, 77, 94, 0.5);
  }

  .btn.recbtn {
    background: var(--rec);
    border-color: var(--rec);
    color: #20060a;
    font-weight: 700;
    height: 40px;
    padding: 0 22px;
  }

  .btn.stop {
    height: 40px;
    padding: 0 22px;
    font-weight: 700;
  }

  .info {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .hint {
    margin: -4px 0 8px;
    font-size: 12px;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(230px, 1fr));
    gap: 8px;
  }

  .src {
    display: grid;
    grid-template-columns: 18px 1fr;
    grid-template-rows: auto auto;
    column-gap: 8px;
    padding: 8px 10px;
    border: 1px solid var(--line);
    border-radius: var(--radius-sm);
    background: var(--panel-2);
    cursor: pointer;
  }

  .src input {
    grid-row: span 2;
    align-self: center;
    margin: 0;
    accent-color: var(--accent);
  }

  .src.on {
    border-color: var(--accent);
    background: var(--accent-soft);
  }

  .src .t {
    font-weight: 600;
  }

  .src .n {
    font-size: 11px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .locked .src {
    cursor: default;
    opacity: 0.8;
  }

  .where {
    display: flex;
    gap: 10px;
    align-items: baseline;
    font-size: 12px;
  }

  code {
    color: var(--text);
    word-break: break-all;
  }
</style>
