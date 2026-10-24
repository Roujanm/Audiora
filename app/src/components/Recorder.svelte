<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import { shortName, stripColor } from '../lib/theme.ts';
  import Icon from './Icon.svelte';
  import LedMeter from './LedMeter.svelte';

  const MAX_SOURCES = 8;

  const strips = $derived(engine.info?.strips ?? []);
  const buses = $derived(engine.info?.buses ?? []);
  let chosen = $state<string[]>(['bus/0']);
  let busy = $state(false);
  let message = $state('');
  let lastTake = $state('');

  const rec = $derived(engine.rec);
  const channels = $derived(chosen.length * 2);

  $effect(() => {
    void engine.refreshTakes().catch(() => {});
  });

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
      lastTake = r.path as string;
    } catch (e) {
      message = String(e instanceof Error ? e.message : e);
    }
    busy = false;
  }

  function timecode(sec: number) {
    const h = Math.floor(sec / 3600);
    const m = Math.floor((sec % 3600) / 60);
    const s = Math.floor(sec % 60);
    const f = Math.floor((sec % 1) * 100);
    const two = (n: number) => String(n).padStart(2, '0');
    return `${h > 0 ? two(h) + ':' : ''}${two(m)}:${two(s)}.${two(f)}`;
  }

  function dur(sec: number) {
    if (sec < 60) return `${sec.toFixed(1)} s`;
    const m = Math.floor(sec / 60);
    return `${m}:${String(Math.floor(sec % 60)).padStart(2, '0')} min`;
  }

  function size(bytes: number) {
    return bytes > 1e9 ? `${(bytes / 1e9).toFixed(2)} GB` : `${(bytes / 1e6).toFixed(bytes > 1e8 ? 0 : 1)} MB`;
  }

  function when(ms: number) {
    const d = new Date(ms);
    const today = new Date();
    const time = d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' });
    return d.toDateString() === today.toDateString() ? `Today ${time}` : `${d.toLocaleDateString([], { day: 'numeric', month: 'short' })} ${time}`;
  }

  const nameOf = (src: string) => {
    const [kind, i] = src.split('/');
    return kind === 'bus'
      ? { title: +i === 0 ? 'Master mix' : `Output ${+i + 1}`, sub: shortName(buses[+i]?.name ?? ''), color: 'var(--accent)' }
      : {
          title: strips[+i]?.kind === 'input' ? `Input ${+i + 1}` : 'Tone',
          sub: shortName(strips[+i]?.name ?? ''),
          color: stripColor(strips[+i], +i),
        };
  };
  const sources = $derived([...strips.map((_, i) => `strip/${i}`), ...buses.map((_, i) => `bus/${i}`)]);
</script>

<div class="rec">
  <section class="panel transport" class:live={rec.recording}>
    <div class="tc">
      <span class="eyebrow">{rec.recording ? 'Recording' : 'Ready'}</span>
      <div class="clock num" class:live={rec.recording}>{timecode(rec.seconds ?? 0)}</div>
      <p class="muted sub">
        {#if rec.recording}
          {(rec.sources?.length ?? 0) * 2} channels · 32-bit float{#if (rec.dropped_frames ?? 0) > 0} · <b class="bad">{rec.dropped_frames} dropped frames</b>{/if}
        {:else}
          {channels} channel{channels === 1 ? '' : 's'} · 32-bit float WAV
        {/if}
      </p>
    </div>

    <div class="actions">
      {#if rec.recording}
        <button class="rbtn stop" onclick={stop} disabled={busy} aria-label="Stop recording"><Icon name="stop" size={26} stroke={2.2} /></button>
        <span class="hint">Stop</span>
      {:else}
        <button class="rbtn go" onclick={start} disabled={busy || chosen.length === 0} aria-label="Start recording"><i></i></button>
        <span class="hint">Record</span>
      {/if}
    </div>
  </section>

  {#if message}<p class="chip bad msg" role="alert">{message}</p>{/if}
  {#if rec.error}<p class="chip bad msg" role="alert">{rec.error}</p>{/if}

  <section class="panel tracks" class:locked={rec.recording}>
    <header>
      <h2 class="eyebrow">Tracks</h2>
      <span class="muted">Each source is one stereo track. Inputs are recorded raw (before gain and EQ); outputs are what you hear.</span>
    </header>
    <div class="grid">
      {#each sources as id (id)}
        {@const n = nameOf(id)}
        {@const on = chosen.includes(id)}
        <label class="src" class:on style:--c={n.color}>
          <input type="checkbox" checked={on} disabled={rec.recording} onchange={() => toggle(id)} />
          <span class="box" aria-hidden="true">{#if on}<Icon name="check" size={13} stroke={2.6} />{/if}</span>
          <span class="txt">
            <strong>{n.title}</strong>
            <span class="muted">{n.sub}</span>
          </span>
          <span class="m">
            <LedMeter
              orientation="horizontal"
              scale={false}
              thickness={5}
              levels={[engine.peakDb(`${id}/peak_l`), engine.peakDb(`${id}/peak_r`)]}
            />
          </span>
        </label>
      {/each}
    </div>
  </section>

  <section class="panel takes">
    <header>
      <h2 class="eyebrow">Recent takes</h2>
      <button class="btn sm ghost" onclick={() => engine.refreshTakes()}>Refresh</button>
    </header>
    <p class="folder muted num" title={engine.info?.recordings_dir}><Icon name="folder" size={13} /> {engine.info?.recordings_dir}</p>
    {#each engine.takes as t (t.path)}
      <div class="take" class:fresh={t.path === lastTake}>
        <span class="wavico"><Icon name="wave" size={16} /></span>
        <div class="tn">
          <strong class="num">{t.name}</strong>
          <span class="muted">{when(t.modified_ms)}</span>
        </div>
        <span class="num">{dur(t.seconds)}</span>
        <span class="chip">{t.channels} ch · {t.sample_rate / 1000} kHz</span>
        <span class="num muted">{size(t.bytes)}</span>
        {#if engine.canReveal}
          <button class="btn sm" onclick={() => engine.revealTake(t.path).catch((e) => (message = String(e)))}>Show in folder</button>
        {/if}
      </div>
    {:else}
      <p class="empty muted">No recordings yet. Press record to make your first take.</p>
    {/each}
  </section>
</div>

<style>
  .rec {
    display: flex;
    flex-direction: column;
    gap: 16px;
    max-width: 1000px;
  }

  .transport {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 24px;
    padding: 22px 28px;
  }

  .transport.live {
    border-color: color-mix(in oklab, var(--rec) 45%, transparent);
    background:
      radial-gradient(500px 160px at 0% 50%, color-mix(in oklab, var(--rec) 13%, transparent), transparent 70%),
      linear-gradient(180deg, var(--bg-2), var(--bg-1));
  }

  .clock {
    font-size: 54px;
    font-weight: 500;
    line-height: 1.1;
    letter-spacing: -0.02em;
    color: var(--text-2);
    margin: 4px 0 2px;
  }

  .clock.live {
    color: var(--text);
    text-shadow: 0 0 28px color-mix(in oklab, var(--rec) 50%, transparent);
  }

  .sub {
    font-size: 12.5px;
  }

  .bad {
    color: var(--bad);
  }

  .actions {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 8px;
  }

  .hint {
    font-size: 11px;
    font-weight: 600;
    color: var(--muted);
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .rbtn {
    width: 76px;
    height: 76px;
    border-radius: 50%;
    border: 0;
    display: grid;
    place-items: center;
    background: linear-gradient(180deg, var(--bg-4), var(--bg-3));
    box-shadow: 0 0 0 1px var(--line-3), 0 10px 24px -8px rgba(0, 0, 0, 0.8), 0 1px 0 rgba(255, 255, 255, 0.12) inset;
    transition: transform 0.1s, box-shadow 0.2s;
  }

  .rbtn:active:not(:disabled) {
    transform: scale(0.96);
  }

  .rbtn:disabled {
    opacity: 0.4;
    cursor: default;
  }

  .rbtn.go i {
    width: 34px;
    height: 34px;
    border-radius: 50%;
    background: radial-gradient(circle at 35% 30%, #ff8a98, var(--rec) 55%, #b11a2e);
    box-shadow: 0 0 22px color-mix(in oklab, var(--rec) 65%, transparent);
  }

  .rbtn.go:hover:not(:disabled) {
    box-shadow: 0 0 0 1px var(--rec), 0 10px 30px -6px color-mix(in oklab, var(--rec) 55%, transparent);
  }

  .rbtn.stop {
    color: #fff;
    background: linear-gradient(180deg, #ff6275, var(--rec));
    box-shadow: 0 0 0 1px var(--rec), 0 0 34px -4px var(--rec);
    animation: breathe 1.6s infinite ease-in-out;
  }

  @keyframes breathe {
    50% {
      box-shadow: 0 0 0 1px var(--rec), 0 0 14px -4px var(--rec);
    }
  }

  .msg {
    height: auto;
    padding: 6px 12px;
    white-space: normal;
    align-self: flex-start;
  }

  .tracks,
  .takes {
    padding: 8px 8px 10px;
  }

  header {
    display: flex;
    align-items: baseline;
    gap: 14px;
    padding: 10px 14px 8px;
  }

  header span {
    font-size: 12px;
    flex: 1;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(290px, 1fr));
    gap: 8px;
    padding: 4px 6px 6px;
  }

  .src {
    position: relative;
    display: grid;
    grid-template-columns: 22px 1fr;
    grid-template-rows: auto auto;
    column-gap: 11px;
    row-gap: 8px;
    padding: 11px 13px;
    border-radius: var(--r-md);
    border: 1px solid var(--line);
    background: var(--bg-2);
    cursor: pointer;
    transition: all 0.14s;
  }

  .src:hover {
    border-color: var(--line-3);
  }

  .src.on {
    border-color: color-mix(in oklab, var(--c) 55%, transparent);
    background: linear-gradient(180deg, color-mix(in oklab, var(--c) 10%, transparent), var(--bg-2));
  }

  .src input {
    position: absolute;
    opacity: 0;
    inset: 0;
    cursor: pointer;
  }

  .src input:focus-visible + .box {
    outline: 2px solid var(--accent);
    outline-offset: 2px;
  }

  .box {
    grid-row: 1;
    width: 22px;
    height: 22px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    background: var(--bg-0);
    border: 1px solid var(--line-3);
    color: #05110b;
    transition: all 0.14s;
  }

  .src.on .box {
    background: var(--c);
    border-color: transparent;
    box-shadow: 0 0 12px -2px var(--c);
  }

  .txt {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.25;
  }

  .txt strong {
    font-weight: 600;
  }

  .txt span {
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .m {
    grid-column: 1 / -1;
  }

  .locked .src {
    cursor: default;
    opacity: 0.85;
  }

  .locked .src input {
    cursor: default;
  }

  .folder {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 0 14px 8px;
    font-size: 11px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .take {
    display: grid;
    grid-template-columns: 32px minmax(180px, 1.6fr) 80px auto 80px auto;
    align-items: center;
    gap: 14px;
    padding: 10px 14px;
    border-radius: var(--r-md);
    font-size: 12.5px;
  }

  .take:hover {
    background: var(--bg-3);
  }

  .take.fresh {
    background: var(--accent-soft);
  }

  .wavico {
    width: 30px;
    height: 30px;
    display: grid;
    place-items: center;
    border-radius: 9px;
    color: var(--accent-hi);
    background: var(--accent-soft);
  }

  .tn {
    display: flex;
    flex-direction: column;
    min-width: 0;
    line-height: 1.3;
  }

  .tn strong {
    font-size: 12.5px;
    font-weight: 550;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tn span {
    font-size: 11px;
  }

  .empty {
    padding: 24px;
    text-align: center;
  }
</style>
