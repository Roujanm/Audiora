<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';

  interface Props {
    strip: number;
    color: string;
    /** Highest mic gain / volume boost the channel allows, in dB. */
    boostMax: number;
  }
  let { strip, color, boostMax }: Props = $props();

  const p = (leaf: string) => `strip/${strip}/${leaf}`;
  const g = (leaf: string, d = 0) => engine.get(p(leaf), d);
  const on = (leaf: string) => g(leaf) > 0.5;

  const inDb = $derived(engine.peakDb(p('in_peak')));
  const inHold = $derived(engine.holdDb(p('in_peak')));
  const outDb = $derived(Math.max(engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))));
  const flowing = $derived(Math.max(0, Math.min(1, (outDb + 60) / 54)));

  // a meter bar's fill for a level in dBFS (-60 .. 0)
  const fill = (db: number) => `${Math.max(0, Math.min(100, ((db + 60) / 60) * 100))}%`;
  const grFill = (gr: number) => `${Math.max(0, Math.min(100, (gr / 12) * 100))}%`;
  const fmtDb = (db: number) => (db <= -90 ? '−∞' : `${db > 0 ? '+' : db < 0 ? '−' : ''}${Math.abs(db).toFixed(db > -10 ? 1 : 0)}`);

  const eqActive = $derived([0, 1, 2, 3, 4, 5].filter((b) => Math.abs(g(`eq/${b}/gain_db`)) > 0.05).length);

  type Stage = {
    id: string;
    name: string;
    toggle?: string;
    detail: string;
    meter?: { kind: 'level' | 'gr'; value: number };
    led?: boolean;
  };

  const stages: Stage[] = $derived([
    { id: 'in', name: 'Input', detail: `${fmtDb(inDb)} dB`, meter: { kind: 'level', value: inDb } },
    { id: 'lowcut', name: 'Low cut', toggle: 'lowcut/enabled', detail: `${Math.round(g('lowcut/freq', 80))} Hz` },
    { id: 'denoise', name: 'ClearCast', toggle: 'denoise', detail: `${Math.round(g('denoise_mix', 1) * 100)}%` },
    { id: 'gate', name: 'Gate', toggle: 'gate/enabled', detail: engine.gauge(p('gate/open')) > 0.5 ? 'open' : 'closed', led: engine.gauge(p('gate/open')) > 0.5 },
    { id: 'eq', name: 'EQ', detail: eqActive ? `${eqActive} band${eqActive > 1 ? 's' : ''}` : 'flat' },
    { id: 'deess', name: 'De-esser', toggle: 'deess/enabled', detail: grText(engine.gauge(p('deess/gr_db'))), meter: { kind: 'gr', value: engine.gauge(p('deess/gr_db')) } },
    { id: 'smart', name: 'Smart Vol', toggle: 'smart/enabled', detail: `${fmtDb(engine.gauge(p('smart/gain_db')))} dB` },
    { id: 'comp', name: 'Comp', toggle: 'comp/enabled', detail: grText(engine.gauge(p('comp/gr_db'))), meter: { kind: 'gr', value: engine.gauge(p('comp/gr_db')) } },
    { id: 'limiter', name: 'Limiter', toggle: 'limiter/enabled', detail: grText(engine.gauge(p('limiter/gr_db'))), meter: { kind: 'gr', value: engine.gauge(p('limiter/gr_db')) } },
    { id: 'out', name: 'Output', detail: `${fmtDb(outDb)} dB`, meter: { kind: 'level', value: outDb } },
  ]);

  function grText(gr: number) {
    return gr > 0.3 ? `−${gr.toFixed(1)} dB` : 'idle';
  }

  // gain staging: speech should peak around -10 dBFS when it enters the chain
  const TARGET = -10;
  const advice = $derived(
    inHold <= -90
      ? 'No signal yet'
      : inHold < -24
        ? `Quiet input: about ${Math.round(TARGET - inHold)} dB more gain would help`
        : inHold > -2
          ? 'Input is close to clipping'
          : 'Input level is healthy',
  );
  const adviceTone = $derived(inHold <= -90 ? 'idle' : inHold < -24 || inHold > -2 ? 'warn' : 'ok');

  let matching = $state(0); // seconds left while measuring, 0 = idle
  let peakSeen = -120;
  let timer: ReturnType<typeof setInterval> | undefined;

  function levelMatch() {
    if (matching) return;
    peakSeen = -120;
    matching = 4;
    let ticks = 0;
    timer = setInterval(() => {
      peakSeen = Math.max(peakSeen, engine.peakDb(p('in_peak')));
      ticks++;
      if (ticks % 10 === 0) matching = Math.max(0, matching - 1);
      if (ticks >= 40) {
        clearInterval(timer);
        matching = 0;
        if (peakSeen < -70) return; // nothing was said
        const next = Math.max(0, Math.min(boostMax, g('boost_db') + (TARGET - peakSeen)));
        engine.set(p('boost_db'), Math.round(next * 2) / 2);
      }
    }, 100);
  }

  $effect(() => () => clearInterval(timer));

  // before / after: the last few seconds of the level entering and leaving the chain
  const HISTORY_S = 8;
  let canvas = $state<HTMLCanvasElement>();
  $effect(() => {
    const c = canvas;
    if (!c) return;
    const ctx = c.getContext('2d');
    if (!ctx) return;
    const cols = 240;
    const ins = new Float32Array(cols).fill(-90);
    const outs = new Float32Array(cols).fill(-90);
    let head = 0;
    let last = performance.now();
    let raf = 0;
    const step = (HISTORY_S * 1000) / cols;
    const draw = (now: number) => {
      while (now - last >= step) {
        last += step;
        ins[head] = engine.peakDb(p('in_peak'));
        outs[head] = Math.max(engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r')));
        head = (head + 1) % cols;
      }
      const dpr = window.devicePixelRatio || 1;
      const w = c.clientWidth;
      const h = c.clientHeight;
      if (c.width !== Math.round(w * dpr) || c.height !== Math.round(h * dpr)) {
        c.width = Math.round(w * dpr);
        c.height = Math.round(h * dpr);
      }
      ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
      ctx.clearRect(0, 0, w, h);
      const y = (db: number) => h - Math.max(0, Math.min(1, (db + 60) / 60)) * h;
      // reference lines: -6, -18, -36 dBFS
      ctx.strokeStyle = 'rgba(255,255,255,0.06)';
      ctx.lineWidth = 1;
      for (const db of [-6, -18, -36]) {
        ctx.beginPath();
        ctx.moveTo(0, Math.round(y(db)) + 0.5);
        ctx.lineTo(w, Math.round(y(db)) + 0.5);
        ctx.stroke();
      }
      const accent = getComputedStyle(c).getPropertyValue('--c').trim() || '#4d9bff';
      const plot = (buf: Float32Array, fillStyle: string | CanvasGradient, stroke: string, fillAlpha = 1) => {
        ctx.beginPath();
        ctx.moveTo(0, h);
        for (let i = 0; i < cols; i++) ctx.lineTo((i / (cols - 1)) * w, y(buf[(head + i) % cols]));
        ctx.lineTo(w, h);
        ctx.closePath();
        ctx.fillStyle = fillStyle;
        ctx.globalAlpha = fillAlpha;
        ctx.fill();
        ctx.globalAlpha = 1;
        ctx.beginPath();
        for (let i = 0; i < cols; i++) ctx.lineTo((i / (cols - 1)) * w, y(buf[(head + i) % cols]));
        ctx.strokeStyle = stroke;
        ctx.lineWidth = 1.5;
        ctx.stroke();
      };
      plot(ins, 'rgba(180,189,202,0.10)', 'rgba(180,189,202,0.35)');
      // canvas colours cannot use color-mix(): fade the channel colour with alpha instead
      const grad = ctx.createLinearGradient(0, 0, 0, h);
      grad.addColorStop(0, accent);
      grad.addColorStop(1, 'rgba(0,0,0,0)');
      plot(outs, grad, accent, 0.4);
      raf = requestAnimationFrame(draw);
    };
    raf = requestAnimationFrame(draw);
    return () => cancelAnimationFrame(raf);
  });
</script>

<section class="panel chain" style:--c={color} style:--flow={flowing} aria-label="Signal chain">
  <header>
    <div class="title">
      <p class="eyebrow">Signal chain</p>
      <p class="advice {adviceTone}"><i></i>{matching ? `Speak normally… ${matching}` : advice}</p>
    </div>
    <div class="readouts">
      <div class="ro"><span class="eyebrow">In</span><span class="num">{fmtDb(inHold)}</span></div>
      <div class="ro"><span class="eyebrow">Out</span><span class="num">{fmtDb(outDb)}</span></div>
      <button class="btn sm" class:busy={matching > 0} onclick={levelMatch} disabled={matching > 0} title="Speak for four seconds; the gain is set so your voice enters the chain at the right level">
        {matching ? 'Listening…' : 'Set level'}
      </button>
    </div>
  </header>

  <div class="history">
    <canvas bind:this={canvas} aria-hidden="true"></canvas>
    <div class="legend">
      <span><i class="raw"></i>Before</span>
      <span><i class="processed"></i>After</span>
    </div>
    <span class="scale s6">−6</span><span class="scale s18">−18</span><span class="scale s36">−36</span>
  </div>

  <ol class="stages">
    {#each stages as s (s.id)}
      {@const enabled = s.toggle ? on(s.toggle) : true}
      <li class="stage" class:off={!enabled} class:endpoint={s.id === 'in' || s.id === 'out'}>
        <svelte:element
          this={s.toggle ? 'button' : 'div'}
          class="node"
          role={s.toggle ? undefined : 'group'}
          aria-pressed={s.toggle ? enabled : undefined}
          aria-label={s.toggle ? `${s.name}: ${enabled ? 'on' : 'off'}` : s.name}
          title={s.toggle ? `${enabled ? 'Bypass' : 'Enable'} ${s.name}` : undefined}
          onclick={s.toggle ? () => engine.set(p(s.toggle!), !enabled) : undefined}
        >
          <span class="name">{s.name}</span>
          {#if s.meter}
            <span class="bar {s.meter.kind}" aria-hidden="true"><i style:width={s.meter.kind === 'gr' ? grFill(s.meter.value) : fill(s.meter.value)}></i></span>
          {:else if s.led !== undefined}
            <span class="led" class:lit={enabled && s.led} aria-hidden="true"></span>
          {:else}
            <span class="dot" aria-hidden="true"></span>
          {/if}
          <span class="detail num">{enabled ? s.detail : 'bypassed'}</span>
        </svelte:element>
      </li>
    {/each}
  </ol>
</section>

<style>
  .chain {
    padding: 16px 18px 18px;
    overflow: hidden;
  }

  /* the channel colour washes in from the left, stronger while signal flows */
  .chain::before {
    content: '';
    position: absolute;
    inset: 0;
    pointer-events: none;
    background: radial-gradient(600px 160px at 0% 0%, color-mix(in oklab, var(--c) calc(6% + var(--flow) * 10%), transparent), transparent 70%);
    transition: background 0.2s linear;
  }

  header {
    position: relative;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    margin-bottom: 14px;
  }

  .advice {
    display: flex;
    align-items: center;
    gap: 7px;
    margin-top: 3px;
    font-size: 13px;
    color: var(--text-2);
  }

  .advice i {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--faint);
  }

  .advice.ok i {
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }

  .advice.warn i {
    background: var(--warn);
    box-shadow: 0 0 8px var(--warn);
  }

  .readouts {
    display: flex;
    align-items: center;
    gap: 14px;
  }

  .ro {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    min-width: 54px;
  }

  .ro .num {
    font-size: 17px;
    font-weight: 600;
    color: var(--text);
  }

  .btn.busy {
    border-color: color-mix(in oklab, var(--c) 60%, transparent);
    color: var(--text);
  }

  .history {
    position: relative;
    height: 96px;
    margin-bottom: 12px;
    border-radius: var(--r-md);
    background: var(--bg-0);
    border: 1px solid var(--line);
    box-shadow: inset 0 2px 8px rgba(0, 0, 0, 0.5);
    overflow: hidden;
  }

  canvas {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    --c: inherit;
  }

  .legend {
    position: absolute;
    top: 8px;
    left: 10px;
    display: flex;
    gap: 12px;
    font-size: 11px;
    color: var(--muted);
  }

  .legend span {
    display: flex;
    align-items: center;
    gap: 5px;
  }

  .legend i {
    width: 10px;
    height: 3px;
    border-radius: 2px;
  }

  .legend .raw {
    background: rgba(180, 189, 202, 0.6);
  }

  .legend .processed {
    background: var(--c);
    box-shadow: 0 0 6px var(--c);
  }

  .scale {
    position: absolute;
    right: 8px;
    font: 10px var(--mono);
    color: var(--faint);
    transform: translateY(-50%);
  }

  /* y = (1 - (db + 60) / 60) * 100% */
  .s6 {
    top: 10%;
  }

  .s18 {
    top: 30%;
  }

  .s36 {
    top: 60%;
  }

  .stages {
    position: relative;
    display: grid;
    grid-template-columns: repeat(10, minmax(0, 1fr));
    gap: 10px;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .stage {
    position: relative;
  }

  /* the wire to the next stage, with signal running along it */
  .stage:not(:last-child)::after {
    content: '';
    position: absolute;
    top: 50%;
    right: -10px;
    width: 10px;
    height: 2px;
    background: linear-gradient(90deg, transparent 0 20%, var(--c) 50%, transparent 80% 100%) 0 0 / 20px 2px repeat-x;
    opacity: calc(0.18 + var(--flow) * 0.82);
    animation: flow 0.6s linear infinite;
  }

  @keyframes flow {
    to {
      background-position: 20px 0;
    }
  }

  .node {
    width: 100%;
    height: 78px;
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    gap: 6px;
    padding: 10px 10px 9px;
    text-align: left;
    background: linear-gradient(180deg, var(--bg-3), var(--bg-2));
    border: 1px solid var(--line-2);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-1), 0 0 0 0 transparent;
    transition:
      border-color 0.15s var(--ease),
      box-shadow 0.2s var(--ease),
      opacity 0.15s var(--ease),
      transform 0.15s var(--ease);
  }

  button.node {
    cursor: pointer;
  }

  button.node:hover {
    transform: translateY(-1px);
    border-color: color-mix(in oklab, var(--c) 45%, var(--line-2));
  }

  .stage:not(.off) .node {
    border-color: color-mix(in oklab, var(--c) 28%, var(--line-2));
    box-shadow: var(--shadow-1), 0 6px 18px -10px color-mix(in oklab, var(--c) 70%, transparent);
  }

  .stage.endpoint .node {
    background: linear-gradient(180deg, color-mix(in oklab, var(--c) 12%, var(--bg-3)), var(--bg-2));
  }

  .stage.off .node {
    opacity: 0.45;
    box-shadow: none;
  }

  .name {
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .detail {
    font-size: 11px;
    color: var(--muted);
    white-space: nowrap;
  }

  .bar {
    position: relative;
    height: 5px;
    border-radius: 3px;
    background: var(--bg-0);
    overflow: hidden;
  }

  .bar i {
    position: absolute;
    inset: 0 auto 0 0;
    border-radius: 3px;
    transition: width 60ms linear;
  }

  .bar.level i {
    background: linear-gradient(90deg, color-mix(in oklab, var(--c) 70%, var(--ok)), var(--c));
  }

  .bar.gr i {
    background: var(--warn);
  }

  .led,
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--bg-0);
    border: 1px solid var(--line-2);
  }

  .led.lit {
    background: var(--c);
    border-color: transparent;
    box-shadow: 0 0 10px var(--c);
  }

  .dot {
    background: color-mix(in oklab, var(--c) 50%, var(--bg-0));
    border-color: transparent;
  }

  @media (prefers-reduced-motion: reduce) {
    .stage::after {
      animation: none;
    }
  }
</style>
