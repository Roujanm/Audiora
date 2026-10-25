<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';
  import {
    formatTime,
    nextTrackId,
    projectEnd,
    rulerStep,
    SOURCES,
    splitClip,
    TRACK_COLORS,
    type Clip,
    type PeakData,
    type Project,
    type Track,
  } from '../lib/studio.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';
  import TextEdit from './TextEdit.svelte';

  const LANE_H = 92;
  const RULER_H = 30;

  let projects = $state<{ name: string; modified_ms: number }[]>([]);
  let project = $state<Project | null>(null);
  let rate = $state(48000);
  let transport = $state<'stopped' | 'playing' | 'recording'>('stopped');
  let position = $state(0);
  let pps = $state(60); // pixels per second
  let scrollX = $state(0);
  let selected = $state<{ track: string; index: number } | null>(null);
  let peaks = $state<Record<string, PeakData>>({});
  let saving = $state<'saved' | 'saving' | 'dirty'>('saved');
  let newName = $state('');
  let naming = $state(false);

  // live waveform while recording: track id -> [frame, linear peak]
  let live = $state<Record<string, [number, number][]>>({});
  let recordStart = 0;

  let anchor = { pos: 0, at: 0 }; // last position the engine reported and when
  let poll = 0;
  let raf = 0;

  // ---- project loading and saving ----------------------------------------------------------
  async function refreshList() {
    try {
      projects = ((await engine.call('studio_list')).projects as typeof projects) ?? [];
    } catch {
      projects = [];
    }
  }

  async function open(name: string) {
    try {
      const r = await engine.call('studio_load', { name });
      project = r.project as Project;
      rate = (r.sample_rate as number) || 48000;
      position = 0;
      selected = null;
      peaks = {};
      try {
        localStorage.setItem('audiora.studio.project', name);
      } catch {
        /* not remembered */
      }
      for (const t of project.tracks) for (const c of t.clips) void loadPeaks(c.take);
      await refreshList();
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  $effect(() => {
    if (!engine.connected || project) return;
    void (async () => {
      await refreshList();
      let last = '';
      try {
        last = localStorage.getItem('audiora.studio.project') ?? '';
      } catch {
        /* none */
      }
      await open(last || projects[0]?.name || 'My first session');
    })();
  });

  let saveTimer = 0;
  function changed() {
    saving = 'dirty';
    clearTimeout(saveTimer);
    saveTimer = window.setTimeout(() => void save(), 400);
  }

  async function save() {
    if (!project) return;
    saving = 'saving';
    try {
      await engine.call('studio_save', { name: project.name, project: $state.snapshot(project) });
      saving = 'saved';
    } catch (e) {
      saving = 'dirty';
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  async function loadPeaks(take: string) {
    if (!project || peaks[take]) return;
    try {
      const p = (await engine.call('studio_peaks', { name: project.name, take })) as unknown as PeakData;
      peaks = { ...peaks, [take]: p };
    } catch {
      /* drawn without a waveform */
    }
  }

  // ---- transport --------------------------------------------------------------------------
  function startPolling() {
    clearInterval(poll);
    poll = window.setInterval(async () => {
      try {
        const s = await engine.call('studio_status');
        const st = s.state as typeof transport;
        if (typeof s.position === 'number') anchor = { pos: s.position, at: performance.now() };
        if (st === 'stopped' && transport === 'playing') {
          transport = 'stopped';
          clearInterval(poll);
        }
      } catch {
        clearInterval(poll);
      }
    }, 150);
    cancelAnimationFrame(raf);
    const tick = () => {
      if (transport === 'stopped') return;
      position = anchor.pos + ((performance.now() - anchor.at) / 1000) * rate;
      if (transport === 'recording') sampleLive();
      followPlayhead();
      raf = requestAnimationFrame(tick);
    };
    raf = requestAnimationFrame(tick);
  }

  async function play() {
    if (!project) return;
    if (transport !== 'stopped') return stop();
    try {
      await save();
      await engine.call('studio_play', { name: project.name, start: Math.round(position) });
      anchor = { pos: position, at: performance.now() };
      transport = 'playing';
      startPolling();
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  async function record() {
    if (!project || transport === 'recording') return;
    const armed = project.tracks.filter((t) => t.armed);
    if (!armed.length) return toast('Arm a track first (the round button on a track)', 'error');
    try {
      if (transport === 'playing') await engine.call('studio_stop');
      await save();
      const start = Math.round(position);
      await engine.call('studio_record', { name: project.name, start, tracks: armed.map((t) => ({ id: t.id, source: t.source })) });
      recordStart = start;
      live = Object.fromEntries(armed.map((t) => [t.id, []]));
      anchor = { pos: start, at: performance.now() };
      transport = 'recording';
      startPolling();
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  async function stop() {
    clearInterval(poll);
    cancelAnimationFrame(raf);
    const was = transport;
    transport = 'stopped';
    try {
      const r = await engine.call('studio_stop');
      if (was === 'recording' && r.take && project) {
        const lanes = r.lanes as string[];
        lanes.forEach((id, lane) => {
          const t = project!.tracks.find((x) => x.id === id);
          if (t) t.clips.push({ take: r.take as string, lane, start: r.start as number, offset: 0, length: r.frames as number });
        });
        live = {};
        changed();
        void loadPeaks(r.take as string);
        toast(`Recorded ${formatTime(r.frames as number, rate)}`, 'ok');
      }
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  function sampleLive() {
    if (!project) return;
    const next = { ...live };
    for (const t of project.tracks) {
      if (!t.armed || !next[t.id]) continue;
      const src = SOURCES.find((s) => s.id === t.source);
      const m = src?.meter ?? 'strip/4';
      const db = Math.max(engine.peakDb(`${m}/peak_l`), engine.peakDb(`${m}/peak_r`));
      next[t.id] = [...next[t.id], [position, Math.pow(10, db / 20)]];
    }
    live = next;
  }

  async function exportMix() {
    if (!project) return;
    try {
      await save();
      const r = await engine.call('studio_export', { name: project.name });
      toast(`Exported ${formatTime((r.seconds as number) * rate, rate)} → ${String(r.path).split('\\').pop()}`, 'ok');
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  async function createProject() {
    const name = newName.trim();
    if (!name) return;
    naming = false;
    newName = '';
    await open(name);
  }

  // ---- editing ----------------------------------------------------------------------------
  function addTrack() {
    if (!project) return;
    const i = project.tracks.length;
    project.tracks.push({
      id: nextTrackId(project),
      name: `Track ${i + 1}`,
      source: 'post/4',
      color: TRACK_COLORS[i % TRACK_COLORS.length],
      gain_db: 0,
      mute: false,
      solo: false,
      armed: false,
      clips: [],
    });
    changed();
  }

  function removeTrack(t: Track) {
    if (!project) return;
    if (t.clips.length && !confirm(`Delete “${t.name}” and its ${t.clips.length} clip(s) from the project? The recordings stay on disk.`)) return;
    project.tracks = project.tracks.filter((x) => x !== t);
    changed();
  }

  function deleteSelected() {
    if (!project || !selected) return;
    const t = project.tracks.find((x) => x.id === selected!.track);
    if (!t) return;
    t.clips.splice(selected.index, 1);
    selected = null;
    changed();
  }

  function splitAtPlayhead() {
    if (!project) return;
    let did = false;
    for (const t of project.tracks) {
      const targets = selected ? (selected.track === t.id ? [selected.index] : []) : t.clips.map((_, i) => i);
      for (const i of targets.sort((a, b) => b - a)) {
        const r = splitClip(t.clips[i], position);
        if (r) {
          t.clips.splice(i, 1, r[0], r[1]);
          did = true;
        }
      }
    }
    if (did) {
      selected = null;
      changed();
    }
  }

  // ---- timeline drawing -------------------------------------------------------------------
  let canvas = $state<HTMLCanvasElement>();
  let scroller = $state<HTMLDivElement>();
  let viewW = $state(800);

  const duration = $derived(project ? Math.max(projectEnd(project), position, rate * 60) : rate * 60);
  const extent = $derived((duration / rate) * pps + 400);
  const viewH = $derived(RULER_H + (project?.tracks.length ?? 0) * LANE_H);
  const x = (frame: number) => (frame / rate) * pps - scrollX;

  $effect(() => {
    if (!scroller) return;
    const ro = new ResizeObserver(([e]) => (viewW = Math.floor(e.contentRect.width)));
    ro.observe(scroller);
    return () => ro.disconnect();
  });

  function followPlayhead() {
    if (!scroller) return;
    const px = (position / rate) * pps;
    if (px > scrollX + viewW - 60) scroller.scrollLeft = px - 120;
  }

  $effect(() => {
    if (!canvas || !project) return;
    const dpr = window.devicePixelRatio || 1;
    const W = viewW;
    const H = viewH;
    canvas.width = W * dpr;
    canvas.height = H * dpr;
    const ctx = canvas.getContext('2d')!;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.clearRect(0, 0, W, H);
    const mono = getComputedStyle(document.documentElement).getPropertyValue('--mono').trim();

    // ruler
    ctx.fillStyle = 'rgba(255,255,255,0.025)';
    ctx.fillRect(0, 0, W, RULER_H);
    const step = rulerStep(pps);
    const first = Math.floor(scrollX / pps / step) * step;
    ctx.font = `500 10px ${mono}`;
    for (let s = first; (s * pps) - scrollX < W; s += step) {
      const gx = Math.round(s * pps - scrollX) + 0.5;
      ctx.strokeStyle = 'rgba(255,255,255,0.18)';
      ctx.beginPath();
      ctx.moveTo(gx, RULER_H - 8);
      ctx.lineTo(gx, RULER_H);
      ctx.stroke();
      ctx.strokeStyle = 'rgba(255,255,255,0.04)';
      ctx.beginPath();
      ctx.moveTo(gx, RULER_H);
      ctx.lineTo(gx, H);
      ctx.stroke();
      ctx.fillStyle = 'rgba(160,170,185,0.8)';
      const m = Math.floor(s / 60);
      const sec = s - m * 60;
      ctx.fillText(`${m}:${(step < 1 ? sec.toFixed(1) : String(Math.round(sec))).padStart(step < 1 ? 4 : 2, '0')}`, gx + 4, 14);
    }

    // lanes and clips
    project.tracks.forEach((t, row) => {
      const y0 = RULER_H + row * LANE_H;
      ctx.fillStyle = row % 2 ? 'rgba(255,255,255,0.012)' : 'rgba(255,255,255,0.025)';
      ctx.fillRect(0, y0, W, LANE_H);
      ctx.strokeStyle = 'rgba(255,255,255,0.05)';
      ctx.beginPath();
      ctx.moveTo(0, y0 + LANE_H - 0.5);
      ctx.lineTo(W, y0 + LANE_H - 0.5);
      ctx.stroke();

      t.clips.forEach((c, i) => {
        const cx = x(c.start);
        const cw = (c.length / rate) * pps;
        if (cx > W || cx + cw < 0) return;
        const sel = selected?.track === t.id && selected.index === i;
        const top = y0 + 6;
        const h = LANE_H - 12;
        ctx.save();
        ctx.beginPath();
        ctx.roundRect(cx, top, Math.max(cw, 2), h, 7);
        ctx.fillStyle = t.color + (t.mute ? '18' : '2c');
        ctx.fill();
        ctx.lineWidth = sel ? 2 : 1;
        ctx.strokeStyle = sel ? '#ffffff' : t.color + 'aa';
        ctx.stroke();
        ctx.clip();
        // waveform
        const pk = peaks[c.take];
        const lane = pk?.lanes[c.lane];
        if (lane && pk) {
          const mid = top + h / 2 + 6;
          const amp = (h - 18) / 2;
          ctx.fillStyle = t.mute ? 'rgba(200,200,200,0.35)' : t.color;
          const framesPerPx = rate / pps;
          const startPx = Math.max(0, -cx);
          const endPx = Math.min(cw, W - cx);
          for (let px = startPx; px < endPx; px++) {
            const f0 = c.offset + px * framesPerPx;
            const b0 = Math.floor(f0 / pk.bin);
            const b1 = Math.max(b0 + 1, Math.floor((f0 + framesPerPx) / pk.bin));
            let lo = 0;
            let hi = 0;
            for (let b = b0; b < b1 && b * 2 + 1 < lane.length; b++) {
              lo = Math.min(lo, lane[b * 2]);
              hi = Math.max(hi, lane[b * 2 + 1]);
            }
            const g = Math.pow(10, t.gain_db / 20);
            ctx.fillRect(cx + px, mid - Math.min(1, hi * g) * amp, 1, Math.max(1, (Math.min(1, hi * g) - Math.max(-1, lo * g)) * amp));
          }
        }
        ctx.fillStyle = 'rgba(255,255,255,0.85)';
        ctx.font = `600 10.5px ${mono}`;
        ctx.fillText(t.name, cx + 8, top + 14);
        ctx.restore();
      });

      // live recording region
      const pts = live[t.id];
      if (transport === 'recording' && pts) {
        const cx = x(recordStart);
        const cw = x(position) - cx;
        ctx.fillStyle = 'rgba(255,65,87,0.14)';
        ctx.strokeStyle = 'rgba(255,65,87,0.8)';
        ctx.beginPath();
        ctx.roundRect(cx, y0 + 6, Math.max(cw, 2), LANE_H - 12, 7);
        ctx.fill();
        ctx.stroke();
        ctx.fillStyle = '#ff6b7d';
        const mid = y0 + LANE_H / 2 + 6;
        const amp = (LANE_H - 30) / 2;
        for (const [f, v] of pts) {
          const px = x(f);
          if (px < 0 || px > W) continue;
          const a = Math.min(1, v) * amp;
          ctx.fillRect(px, mid - a, 2, Math.max(1, a * 2));
        }
      }
    });

    // playhead
    const px = Math.round(x(position)) + 0.5;
    ctx.strokeStyle = transport === 'recording' ? '#ff4157' : '#ffffff';
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.moveTo(px, 0);
    ctx.lineTo(px, H);
    ctx.stroke();
    ctx.fillStyle = ctx.strokeStyle;
    ctx.beginPath();
    ctx.moveTo(px - 6, 0);
    ctx.lineTo(px + 6, 0);
    ctx.lineTo(px, 8);
    ctx.fill();
  });

  // ---- timeline mouse: seek, select, move clips --------------------------------------------
  let drag: { track: Track; index: number; grab: number; startFrame: number; moved: boolean } | null = null;

  function hit(e: PointerEvent) {
    const r = canvas!.getBoundingClientRect();
    const mx = e.clientX - r.left;
    const my = e.clientY - r.top;
    const frame = Math.max(0, ((mx + scrollX) / pps) * rate);
    const row = Math.floor((my - RULER_H) / LANE_H);
    return { mx, my, frame, row };
  }

  function onpointerdown(e: PointerEvent) {
    if (!project) return;
    const h = hit(e);
    canvas!.setPointerCapture(e.pointerId);
    if (h.my < RULER_H || h.row < 0 || h.row >= project.tracks.length) {
      if (transport === 'stopped') position = h.frame;
      selected = null;
      return;
    }
    const t = project.tracks[h.row];
    const i = t.clips.findIndex((c) => h.frame >= c.start && h.frame <= c.start + c.length);
    if (i < 0) {
      selected = null;
      if (transport === 'stopped') position = h.frame;
      return;
    }
    selected = { track: t.id, index: i };
    drag = { track: t, index: i, grab: h.frame - t.clips[i].start, startFrame: t.clips[i].start, moved: false };
  }

  function onpointermove(e: PointerEvent) {
    if (!drag) return;
    const h = hit(e);
    const c = drag.track.clips[drag.index];
    const next = Math.max(0, Math.round(h.frame - drag.grab));
    if (Math.abs(next - drag.startFrame) > rate * 0.01) drag.moved = true;
    if (drag.moved) c.start = next;
  }

  function onpointerup() {
    if (drag?.moved) changed();
    drag = null;
  }

  function onkeydown(e: KeyboardEvent) {
    const tag = (e.target as HTMLElement).tagName;
    if (tag === 'INPUT' || tag === 'SELECT' || tag === 'TEXTAREA') return;
    if (e.code === 'Space') {
      e.preventDefault();
      void play();
    } else if (e.key === 'r' || e.key === 'R') {
      void record();
    } else if (e.key === 'Delete' || e.key === 'Backspace') {
      deleteSelected();
    } else if (e.key === 's' || e.key === 'S') {
      splitAtPlayhead();
    } else if (e.key === 'Home') {
      if (transport === 'stopped') position = 0;
    }
  }

  $effect(() => () => {
    clearInterval(poll);
    cancelAnimationFrame(raf);
    clearTimeout(saveTimer);
  });

  const meterOf = (t: Track) => {
    const m = SOURCES.find((s) => s.id === t.source)?.meter ?? 'strip/4';
    return Math.max(engine.peakDb(`${m}/peak_l`), engine.peakDb(`${m}/peak_r`));
  };
</script>

<svelte:window {onkeydown} />

{#if project}
  <div class="studio">
    <section class="bar panel">
      <div class="proj">
        {#if naming}
          <form onsubmit={(e) => { e.preventDefault(); void createProject(); }}>
            <input type="text" bind:value={newName} placeholder="Project name" maxlength="60" aria-label="New project name" />
            <button class="btn sm primary" type="submit" disabled={!newName.trim()}>Create</button>
            <button class="btn sm ghost" type="button" onclick={() => (naming = false)}>Cancel</button>
          </form>
        {:else}
          <select aria-label="Project" value={project.name} onchange={(e) => void open(e.currentTarget.value)}>
            {#each projects as p (p.name)}<option value={p.name}>{p.name}</option>{/each}
          </select>
          <button class="btn sm ghost" onclick={() => (naming = true)} title="New project"><Icon name="plus" size={14} /> New</button>
          <span class="saved" class:dirty={saving !== 'saved'}>{saving === 'saved' ? 'Saved' : 'Saving…'}</span>
        {/if}
      </div>

      <div class="transport">
        <button class="tbtn" title="Go to start (Home)" aria-label="Go to start" disabled={transport !== 'stopped'} onclick={() => (position = 0)}><Icon name="refresh" size={16} /></button>
        <button class="tbtn big" class:on={transport === 'playing'} title="Play / stop (Space)" aria-label={transport === 'playing' ? 'Stop' : 'Play'} onclick={() => void play()}>
          <Icon name={transport === 'stopped' ? 'play' : 'stop'} size={18} />
        </button>
        <button class="tbtn big rec" class:on={transport === 'recording'} title="Record (R)" aria-label="Record" onclick={() => void (transport === 'recording' ? stop() : record())}>
          <Icon name={transport === 'recording' ? 'stop' : 'record'} size={18} />
        </button>
        <div class="clock num" class:rec={transport === 'recording'}>{formatTime(position, rate)}</div>
      </div>

      <div class="tools">
        <button class="btn sm ghost" title="Split at the playhead (S)" onclick={splitAtPlayhead}><Icon name="sliders" size={14} /> Split</button>
        <button class="btn sm ghost" title="Delete the selected clip (Del)" disabled={!selected} onclick={deleteSelected}><Icon name="trash" size={14} /></button>
        <div class="zoom" role="group" aria-label="Zoom">
          <button class="btn sm ghost" aria-label="Zoom out" onclick={() => (pps = Math.max(8, pps / 1.5))}>−</button>
          <button class="btn sm ghost" aria-label="Zoom in" onclick={() => (pps = Math.min(800, pps * 1.5))}>+</button>
        </div>
        <button class="btn sm primary" onclick={() => void exportMix()}><Icon name="save" size={14} /> Export mix</button>
      </div>
    </section>

    <section class="tracks panel">
      <div class="heads">
        <div class="headspacer" style:height="{RULER_H}px">
          <button class="btn sm ghost" onclick={addTrack}><Icon name="plus" size={13} /> Track</button>
        </div>
        {#each project.tracks as t (t.id)}
          <div class="head" style:height="{LANE_H}px" style:--c={t.color}>
            <i class="stripe"></i>
            <div class="row1">
              <button class="arm" class:on={t.armed} title={t.armed ? 'Armed for recording' : 'Arm for recording'} aria-pressed={t.armed} aria-label="Arm {t.name}" onclick={() => { t.armed = !t.armed; changed(); }}></button>
              <span class="nm"><TextEdit value={t.name} label="Track name" onchange={(v) => { t.name = v || t.name; changed(); }} /></span>
              <button class="mini" class:mute={t.mute} aria-pressed={t.mute} title="Mute" onclick={() => { t.mute = !t.mute; changed(); }}>M</button>
              <button class="mini" class:solo={t.solo} aria-pressed={t.solo} title="Solo" onclick={() => { t.solo = !t.solo; changed(); }}>S</button>
              <button class="mini x" title="Remove track" aria-label="Remove {t.name}" onclick={() => removeTrack(t)}><Icon name="x" size={11} /></button>
            </div>
            <div class="row2">
              <select aria-label="Source of {t.name}" value={t.source} onchange={(e) => { t.source = e.currentTarget.value; changed(); }}>
                {#each SOURCES as s (s.id)}<option value={s.id}>{s.label}</option>{/each}
              </select>
              <span class="lvl" title="Input level"><i style:width="{Math.max(0, Math.min(1, (meterOf(t) + 60) / 60)) * 100}%"></i></span>
            </div>
            <div class="row3">
              <input
                type="range"
                min="-40"
                max="6"
                step="0.5"
                value={t.gain_db}
                style="--fill: {((t.gain_db + 40) / 46) * 100}%"
                aria-label="{t.name} volume"
                oninput={(e) => { t.gain_db = +e.currentTarget.value; changed(); }}
              />
              <output class="num">{t.gain_db > 0 ? '+' : ''}{t.gain_db.toFixed(1)}</output>
            </div>
          </div>
        {/each}
      </div>

      <div class="timeline" bind:this={scroller} onscroll={() => (scrollX = scroller!.scrollLeft)}>
        <div class="extent" style:width="{extent}px" style:height="{viewH}px">
          <canvas
            bind:this={canvas}
            style:width="{viewW}px"
            style:height="{viewH}px"
            aria-label="Timeline"
            {onpointerdown}
            {onpointermove}
            {onpointerup}
            onpointercancel={onpointerup}
          ></canvas>
        </div>
      </div>
    </section>

    <p class="hint muted">
      <kbd>Space</kbd> play · <kbd>R</kbd> record · <kbd>S</kbd> split · <kbd>Del</kbd> delete clip · drag clips to move ·
      click the ruler to move the playhead. Recordings are 32-bit float WAV in Music\Audiora\Studio.
    </p>
  </div>
{:else}
  <div class="loading muted">Opening the studio…</div>
{/if}

<style>
  .studio {
    display: flex;
    flex-direction: column;
    gap: 12px;
    height: 100%;
    min-height: 0;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 16px;
    padding: 10px 14px;
    flex-wrap: wrap;
  }

  .proj,
  .proj form,
  .tools,
  .transport,
  .zoom {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .proj {
    flex: 1;
    min-width: 260px;
  }

  .proj select {
    min-width: 200px;
    font-weight: 600;
  }

  .saved {
    font-size: 11px;
    color: var(--ok);
  }

  .saved.dirty {
    color: var(--muted);
  }

  .transport {
    gap: 10px;
  }

  .tbtn {
    width: 36px;
    height: 36px;
    border-radius: 50%;
    display: grid;
    place-items: center;
    padding: 0;
    border: 1px solid var(--line-2);
    background: var(--bg-2);
    color: var(--text-2);
  }

  .tbtn:hover:not(:disabled) {
    color: var(--text);
    border-color: var(--line-3);
  }

  .tbtn:disabled {
    opacity: 0.4;
  }

  .tbtn.big {
    width: 44px;
    height: 44px;
  }

  .tbtn.on {
    color: var(--on-accent);
    background: var(--accent);
    border-color: transparent;
  }

  .tbtn.rec {
    color: var(--rec);
  }

  .tbtn.rec.on {
    color: #fff;
    background: var(--rec);
    box-shadow: 0 0 20px -4px var(--rec);
    animation: pulse 1.2s infinite;
  }

  @keyframes pulse {
    50% {
      box-shadow: 0 0 4px -2px var(--rec);
    }
  }

  .clock {
    min-width: 120px;
    padding: 6px 14px;
    border-radius: 10px;
    background: var(--bg-0);
    border: 1px solid var(--line);
    font-size: 20px;
    font-weight: 600;
    text-align: center;
  }

  .clock.rec {
    color: var(--rec);
  }

  .tracks {
    flex: 1;
    min-height: 0;
    display: flex;
    overflow: auto;
    padding: 0;
  }

  .heads {
    flex: none;
    width: 250px;
    border-right: 1px solid var(--line);
    background: var(--bg-1);
    position: sticky;
    left: 0;
    z-index: 2;
  }

  .headspacer {
    display: flex;
    align-items: center;
    padding: 0 8px;
    border-bottom: 1px solid var(--line);
  }

  .head {
    position: relative;
    display: flex;
    flex-direction: column;
    justify-content: center;
    gap: 6px;
    padding: 0 10px 0 14px;
    border-bottom: 1px solid var(--line);
  }

  .stripe {
    position: absolute;
    left: 0;
    top: 8px;
    bottom: 8px;
    width: 3px;
    border-radius: 0 3px 3px 0;
    background: var(--c);
  }

  .row1,
  .row2,
  .row3 {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .arm {
    flex: none;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    padding: 0;
    border: 2px solid color-mix(in oklab, var(--rec) 70%, transparent);
    background: none;
  }

  .arm.on {
    background: var(--rec);
    box-shadow: 0 0 10px var(--rec);
  }

  .nm {
    flex: 1;
    min-width: 0;
    font-weight: 650;
    font-size: 13px;
    overflow: hidden;
  }

  .mini {
    flex: none;
    width: 22px;
    height: 22px;
    border-radius: 6px;
    padding: 0;
    display: grid;
    place-items: center;
    font: 700 10px var(--font);
    border: 1px solid var(--line-2);
    background: var(--bg-2);
    color: var(--muted);
  }

  .mini.mute {
    color: #fff;
    background: var(--bad);
    border-color: transparent;
  }

  .mini.solo {
    color: #1a1400;
    background: #ffd24d;
    border-color: transparent;
  }

  .mini.x {
    border-color: transparent;
    background: none;
  }

  .row2 select {
    flex: 1;
    height: 26px;
    font-size: 11.5px;
    min-width: 0;
  }

  .lvl {
    flex: none;
    width: 46px;
    height: 6px;
    border-radius: 3px;
    background: var(--bg-0);
    overflow: hidden;
  }

  .lvl i {
    display: block;
    height: 100%;
    background: linear-gradient(90deg, #3ddc97, #d3e65a 70%, #ff5a6b);
  }

  .row3 input {
    flex: 1;
  }

  .row3 output {
    width: 36px;
    text-align: right;
    font-size: 10.5px;
    color: var(--muted);
  }

  .timeline {
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    overflow-y: hidden;
  }

  .extent {
    position: relative;
  }

  canvas {
    position: sticky;
    left: 0;
    display: block;
    cursor: default;
    touch-action: none;
  }

  .hint {
    font-size: 11.5px;
  }

  kbd {
    font: 600 10px var(--mono);
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--bg-2);
    border: 1px solid var(--line-2);
  }

  .loading {
    margin: 20vh auto;
    text-align: center;
  }
</style>
