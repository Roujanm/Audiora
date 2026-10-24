<script lang="ts">
  import { tick } from 'svelte';
  import { engine } from '../lib/engine.svelte.ts';
  import { linToDb } from '../lib/dsp.ts';
  import { rateLabel, shortName, stripColor } from '../lib/theme.ts';
  import Icon from './Icon.svelte';
  import LedMeter from './LedMeter.svelte';

  const strips = $derived(engine.info?.strips ?? []);
  const buses = $derived(engine.info?.buses ?? []);

  let host: HTMLDivElement;
  let srcPorts: HTMLElement[] = [];
  let busPorts: HTMLElement[] = [];
  let cables = $state<{ d: string; color: string; key: string }[]>([]);
  let size = $state({ w: 800, h: 400 });

  const maskOf = (si: number) => engine.get(`strip/${si}/bus_mask`, 1);

  function toggle(si: number, bi: number) {
    engine.set(`strip/${si}/bus_mask`, maskOf(si) ^ (1 << bi));
  }

  function layout() {
    if (!host) return;
    const hr = host.getBoundingClientRect();
    size = { w: hr.width, h: hr.height };
    const out: typeof cables = [];
    strips.forEach((s, si) => {
      const a = srcPorts[si]?.getBoundingClientRect();
      buses.forEach((_b, bi) => {
        if (((maskOf(si) >> bi) & 1) === 0) return;
        const z = busPorts[bi]?.getBoundingClientRect();
        if (!a || !z) return;
        const x1 = a.left + a.width / 2 - hr.left;
        const y1 = a.top + a.height / 2 - hr.top;
        const x2 = z.left + z.width / 2 - hr.left;
        const y2 = z.top + z.height / 2 - hr.top;
        const dx = (x2 - x1) * 0.55;
        out.push({
          d: `M${x1.toFixed(1)} ${y1.toFixed(1)} C${(x1 + dx).toFixed(1)} ${y1.toFixed(1)} ${(x2 - dx).toFixed(1)} ${y2.toFixed(1)} ${x2.toFixed(1)} ${y2.toFixed(1)}`,
          color: stripColor(s, si),
          key: `${si}-${bi}`,
        });
      });
    });
    cables = out;
  }

  $effect(() => {
    // re-run whenever a route, strip or bus changes
    strips.forEach((_s, i) => maskOf(i));
    void buses.length;
    void tick().then(layout);
  });

  $effect(() => {
    const ro = new ResizeObserver(() => layout());
    ro.observe(host);
    return () => ro.disconnect();
  });
</script>

<div class="patch" bind:this={host}>
  <svg class="cables" width={size.w} height={size.h} viewBox="0 0 {size.w} {size.h}" aria-hidden="true">
    {#each cables as c (c.key)}
      <path d={c.d} class="glow" stroke={c.color} />
      <path d={c.d} class="wire" stroke={c.color} />
    {/each}
  </svg>

  <section class="col" aria-label="Sources">
    <h2 class="eyebrow">Sources</h2>
    {#each strips as s, si (si)}
      {@const color = stripColor(s, si)}
      <div class="card panel" style:--c={color}>
        <span class="badge"><Icon name={s.kind === 'input' ? 'mic' : 'wave'} size={16} /></span>
        <div class="meta">
          <strong>{s.kind === 'input' ? shortName(s.name) : 'Tone generator'}</strong>
          <span class="muted">{s.kind === 'input' ? `Input ${si + 1} · ${rateLabel(s.sample_rate)}` : 'Test signal'}</span>
        </div>
        <div class="sends" role="group" aria-label="Send {s.name} to">
          {#each buses as b, bi (bi)}
            {@const on = ((maskOf(si) >> bi) & 1) === 1}
            <button class="send" class:on aria-pressed={on} title="{on ? 'Stop sending to' : 'Send to'} {b.name}" onclick={() => toggle(si, bi)}>
              {bi === 0 ? 'Master' : `Out ${bi + 1}`}
            </button>
          {/each}
        </div>
        <i class="port" bind:this={srcPorts[si]}></i>
      </div>
    {/each}
  </section>

  <div class="gap" aria-hidden="true"></div>

  <section class="col" aria-label="Outputs">
    <h2 class="eyebrow">Outputs</h2>
    {#each buses as b, bi (bi)}
      <div class="card panel bus" style:--c="var(--accent)">
        <i class="port" bind:this={busPorts[bi]}></i>
        <span class="badge"><Icon name="speaker" size={16} /></span>
        <div class="meta">
          <strong>{shortName(b.name)}</strong>
          <span class="muted">{bi === 0 ? 'Master output' : `Output ${bi + 1}`} · {rateLabel(b.sample_rate)}</span>
        </div>
        <div class="lvl">
          <LedMeter
            orientation="horizontal"
            scale={false}
            thickness={5}
            levels={[engine.peakDb(`bus/${bi}/peak_l`), engine.peakDb(`bus/${bi}/peak_r`)]}
          />
        </div>
      </div>
    {/each}
  </section>
</div>

<style>
  .patch {
    position: relative;
    display: grid;
    grid-template-columns: minmax(340px, 1fr) minmax(120px, 0.55fr) minmax(300px, 0.9fr);
    max-width: 1100px;
    padding: 4px;
  }

  .cables {
    position: absolute;
    inset: 0;
    pointer-events: none;
    z-index: 0;
    overflow: visible;
  }

  .wire,
  .glow {
    fill: none;
    stroke-linecap: round;
  }

  .wire {
    stroke-width: 2.4;
  }

  .glow {
    stroke-width: 8;
    opacity: 0.18;
    filter: blur(3px);
  }

  .col {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .col h2 {
    padding: 0 4px 2px;
  }

  .card {
    position: relative;
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 12px 16px;
    min-height: 68px;
    flex-wrap: wrap;
  }

  .badge {
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: color-mix(in oklab, var(--c) 14%, transparent);
    border: 1px solid color-mix(in oklab, var(--c) 30%, transparent);
  }

  .meta {
    display: flex;
    flex-direction: column;
    min-width: 0;
    flex: 1;
    line-height: 1.3;
  }

  .meta strong {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .meta span {
    font-size: 11.5px;
  }

  .sends {
    display: flex;
    gap: 6px;
  }

  .send {
    height: 28px;
    padding: 0 11px;
    border-radius: 8px;
    border: 1px solid var(--line-2);
    background: var(--bg-2);
    color: var(--muted);
    font-size: 11.5px;
    font-weight: 600;
    transition: all 0.12s;
  }

  .send:hover {
    color: var(--text);
    border-color: var(--line-3);
  }

  .send.on {
    color: var(--c);
    background: color-mix(in oklab, var(--c) 15%, transparent);
    border-color: color-mix(in oklab, var(--c) 55%, transparent);
    box-shadow: 0 0 12px -3px color-mix(in oklab, var(--c) 70%, transparent);
  }

  .port {
    position: absolute;
    top: 50%;
    width: 12px;
    height: 12px;
    margin-top: -6px;
    border-radius: 50%;
    background: var(--bg-0);
    border: 2px solid var(--c);
    box-shadow: 0 0 10px -1px var(--c);
  }

  .card:not(.bus) .port {
    right: -7px;
  }

  .bus .port {
    left: -7px;
  }

  .lvl {
    width: 100%;
  }
</style>
