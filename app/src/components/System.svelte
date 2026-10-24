<script lang="ts">
  import { engine } from '../lib/engine.svelte.ts';

  const s = $derived(engine.stats);
  const nodes = $derived([
    ...(engine.info?.strips.filter((n) => n.kind === 'input') ?? []).map((n) => ({ ...n, dir: 'Input' })),
    ...(engine.info?.buses ?? []).map((n, i) => ({ ...n, dir: i === 0 ? 'Master output' : 'Output' })),
  ]);
  const latencyMs = (n: { sample_rate: number | null; period_frames: number | null }) =>
    n.sample_rate && n.period_frames ? (n.period_frames / n.sample_rate) * 1000 : null;

  const renders = $derived(engine.devices.filter((d) => d.flow === 'render'));
  const captures = $derived(engine.devices.filter((d) => d.flow === 'capture'));
  const lic = $derived(engine.license);
</script>

<div class="grid">
  <section class="card">
    <h2>Active devices</h2>
    <table>
      <thead><tr><th>Role</th><th>Device</th><th>Rate</th><th>Period</th></tr></thead>
      <tbody>
        {#each nodes as n, i (i)}
          <tr>
            <td>{n.dir}</td>
            <td class="dev" title={n.name}>{n.name}</td>
            <td class="num">{n.sample_rate ? `${n.sample_rate / 1000} kHz` : '—'}</td>
            <td class="num">
              {#if latencyMs(n) !== null}{latencyMs(n)!.toFixed(1)} ms{n.exclusive ? ` · EXCLUSIVE ${n.bit_depth}-bit` : n.low_latency ? ' · LL' : ''}{:else}—{/if}
            </td>
          </tr>
        {/each}
      </tbody>
    </table>
  </section>

  <section class="card">
    <h2>Audio health</h2>
    <dl>
      <dt>Callbacks</dt><dd class="num">{s.callbacks.toLocaleString()}</dd>
      <dt>Underruns</dt><dd class="num" class:bad={s.underruns > 0}>{s.underruns}</dd>
      <dt>Drift drops</dt><dd class="num" class:bad={s.drift_drops > 0}>{s.drift_drops}</dd>
      <dt>Dropped blocks</dt><dd class="num" class:bad={s.capture_drops > 0}>{s.capture_drops}</dd>
      <dt>Device errors</dt><dd class="num" class:bad={s.device_errors > 0}>{s.device_errors}</dd>
    </dl>
    <p class="muted note">
      Underruns: a device ran out of audio. Drift drops: two device clocks diverged more than the resampler can trim.
      Device errors: a device disappeared (unplugged, or another app took it exclusively); the engine keeps running
      and reconnects by itself. A few underruns at start-up are normal.
    </p>
  </section>

  <section class="card">
    <h2>License</h2>
    {#if lic}
      {#if lic.state === 'valid'}
        <p><span class="chip ok">Valid</span> {lic.edition} edition · {lic.issued_to}</p>
      {:else if lic.state === 'dev-build'}
        <p><span class="chip warn">Development build</span> License checks are off.</p>
      {:else if lic.state === 'none'}
        <p><span class="chip">Play edition</span> No license file found.</p>
      {:else}
        <p><span class="chip bad">Invalid</span> {lic.error}</p>
      {/if}
    {/if}
  </section>

  <section class="card wide">
    <h2>System audio devices</h2>
    <div class="cols">
      <div>
        <h3>Playback ({renders.length})</h3>
        <ul>
          {#each renders as d (d.id)}
            <li><span class="dev" title={d.name}>{d.name}</span> <span class="muted num">{d.sample_rate / 1000} kHz · {d.channels} ch</span>{#if d.default} <span class="chip">default</span>{/if}</li>
          {/each}
        </ul>
      </div>
      <div>
        <h3>Recording ({captures.length})</h3>
        <ul>
          {#each captures as d (d.id)}
            <li><span class="dev" title={d.name}>{d.name}</span> <span class="muted num">{d.sample_rate / 1000} kHz · {d.channels} ch</span>{#if d.default} <span class="chip">default</span>{/if}</li>
          {/each}
        </ul>
      </div>
    </div>
  </section>
</div>

<style>
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(340px, 1fr));
    gap: 12px;
    align-items: start;
  }

  .wide {
    grid-column: 1 / -1;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-size: 12px;
  }

  th {
    text-align: left;
    font-weight: 600;
    color: var(--muted);
    padding: 0 8px 6px 0;
  }

  td {
    padding: 5px 8px 5px 0;
    border-top: 1px solid var(--line);
  }

  .dev {
    max-width: 320px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  td.dev {
    max-width: 220px;
  }

  dl {
    display: grid;
    grid-template-columns: 1fr auto;
    gap: 4px 12px;
    margin: 0 0 8px;
  }

  dt {
    color: var(--muted);
  }

  dd {
    margin: 0;
    text-align: right;
  }

  dd.bad {
    color: var(--warn);
  }

  .note {
    font-size: 11px;
    margin: 0;
  }

  p {
    margin: 0;
  }

  .cols {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(300px, 1fr));
    gap: 16px;
  }

  h3 {
    margin: 0 0 6px;
    font-size: 12px;
  }

  ul {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    max-height: 260px;
    overflow: auto;
  }

  li {
    display: flex;
    gap: 8px;
    align-items: baseline;
    min-width: 0;
  }

  li .dev {
    flex: 0 1 auto;
  }
</style>
