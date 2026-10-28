<script lang="ts">
  import { APP_CHANNEL_KEYS, channelMeta } from '../lib/channels.ts';
  import { desktop } from '../lib/desktop.svelte.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import type { DeviceEntry } from '../lib/protocol.ts';
  import { shortName } from '../lib/theme.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon, { type IconName } from './Icon.svelte';
  import Switch from './Switch.svelte';

  interface Props {
    /** Which part of the settings to show: each fits the window on its own. */
    view?: 'devices' | 'routing' | 'app';
  }
  let { view = 'devices' }: Props = $props();

  const cfg = $derived(engine.channels?.config);
  const renders = $derived(engine.devices.filter((d) => d.flow === 'render'));
  const captures = $derived(engine.devices.filter((d) => d.flow === 'capture'));

  type Draft = { master_out: string; mic_in: string; mic_out: string; parking: string; mic_raw: boolean; devices: Record<string, string> };
  let draft = $state<Draft | null>(null);
  let busy = $state(false);

  // (re)load the form from the engine whenever its saved settings change
  $effect(() => {
    const c = cfg;
    if (!c) return;
    draft = {
      master_out: c.master_out ?? '',
      mic_in: c.mic_in ?? '',
      mic_out: c.mic_out ?? '',
      parking: c.parking ?? '',
      mic_raw: c.mic_raw ?? true,
      devices: Object.fromEntries(APP_CHANNEL_KEYS.map((k) => [k, c.devices[k] ?? ''])),
    };
  });

  const dirty = $derived.by(() => {
    if (!draft || !cfg) return false;
    return (
      draft.master_out !== (cfg.master_out ?? '') ||
      draft.mic_in !== (cfg.mic_in ?? '') ||
      draft.mic_out !== (cfg.mic_out ?? '') ||
      draft.parking !== (cfg.parking ?? '') ||
      draft.mic_raw !== (cfg.mic_raw ?? true) ||
      APP_CHANNEL_KEYS.some((k) => draft!.devices[k] !== (cfg.devices[k] ?? ''))
    );
  });

  const crowded = (name: string) => /sonar|steelseries|voicemeeter/i.test(name);
  // Same rule as the engine: only virtual outputs can be channels (a real speaker as a channel
  // would record what it plays and send it back into itself).
  const isVirtual = (name: string) => /sonar|voicemeeter|vb-audio|cable|virtual|audiora|steelseries|hi-fi/i.test(name);
  const virtuals = $derived(renders.filter((d) => isVirtual(d.name)));
  const listenable = $derived(renders.filter((d) => !isVirtual(d.name) && d.id !== engine.channels?.parking?.device_id));
  const listening = $derived(draft?.master_out || engine.channels?.master_out.device_id || '');
  const picked = $derived(draft ? APP_CHANNEL_KEYS.map((k) => draft!.devices[k]).filter(Boolean) : []);
  // outputs that can park apps: real outputs that are neither the speakers nor a channel's device
  const parkable = $derived(renders.filter((d) => !isVirtual(d.name) && d.id !== listening));
  const parked = $derived(engine.channels?.parking);
  const duplicate = $derived(new Set(picked).size !== picked.length || (!!listening && picked.includes(listening)));
  const chState = (k: string) => engine.channels?.channels.find((c) => c.key === k);
  const noCapture = $derived(!engine.channels?.channels.some((c) => c.kind === 'apps' && c.online));

  const outIcon = (name: string): IconName => (/head|ear|buds|airpods|quantum|arctis|hyperx/i.test(name) ? 'headphones' : 'speaker');
  // Bluetooth hands-free microphones run at 8-16 kHz and switch the headset to telephone quality
  const handsFree = (d: DeviceEntry) => d.flow === 'capture' && d.sample_rate > 0 && d.sample_rate < 16000;
  const label = (d: DeviceEntry) => shortName(d.name) + (d.default ? ' · Windows default' : '');

  async function apply() {
    if (!draft) return;
    busy = true;
    try {
      await engine.configSet({
        master_out: draft.master_out || null,
        mic_in: draft.mic_in || null,
        mic_out: draft.mic_out || null,
        parking: draft.parking || null,
        mic_raw: draft.mic_raw,
        devices: Object.fromEntries(APP_CHANNEL_KEYS.map((k) => [k, draft!.devices[k] || null])),
      });
      toast('Saved. Restarting the audio engine…', 'ok');
      await engine.restartEngine();
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    } finally {
      busy = false;
    }
  }

  function discard() {
    const c = cfg;
    if (!c || !draft) return;
    draft = { ...draft, master_out: c.master_out ?? '', mic_in: c.mic_in ?? '', mic_out: c.mic_out ?? '', parking: c.parking ?? '', mic_raw: c.mic_raw ?? true, devices: Object.fromEntries(APP_CHANNEL_KEYS.map((k) => [k, c.devices[k] ?? ''])) };
  }
</script>

{#if draft}
  <div class="settings" class:one={view !== 'devices'}>
    {#if view === 'devices'}
    <!-- where you listen -->
    <section class="panel card" aria-labelledby="out-h">
      <header>
        <span class="h-ico"><Icon name="headphones" size={18} /></span>
        <div>
          <h2 id="out-h">Output</h2>
          <p class="muted">Where you hear everything. New devices appear here by themselves.</p>
        </div>
      </header>
      <div class="tiles" role="radiogroup" aria-label="Output">
        <button class="tile" class:on={draft.master_out === ''} role="radio" aria-checked={draft.master_out === ''} onclick={() => (draft!.master_out = '')}>
          <span class="t-ico"><Icon name="refresh" size={18} /></span>
          <span class="t-txt"><strong>Automatic</strong><small>Follows Windows{engine.channels?.master_out.auto && engine.channels.master_out.device_name ? ` · now ${shortName(engine.channels.master_out.device_name)}` : ''}</small></span>
        </button>
        {#each listenable as d (d.id)}
          <button class="tile" class:on={draft.master_out === d.id} role="radio" aria-checked={draft.master_out === d.id} onclick={() => (draft!.master_out = d.id)}>
            <span class="t-ico"><Icon name={outIcon(d.name)} size={18} /></span>
            <span class="t-txt"><strong>{shortName(d.name)}</strong><small>{d.sample_rate / 1000} kHz{d.default ? ' · Windows default' : ''}</small></span>
          </button>
        {/each}
      </div>
    </section>

    <!-- microphone -->
    <section class="panel card" aria-labelledby="mic-h">
      <header>
        <span class="h-ico"><Icon name="mic" size={18} /></span>
        <div>
          <h2 id="mic-h">Microphone</h2>
          <p class="muted">The microphone the Voice page cleans up.</p>
        </div>
      </header>
      <div class="tiles" role="radiogroup" aria-label="Microphone">
        <button class="tile" class:on={draft.mic_in === ''} role="radio" aria-checked={draft.mic_in === ''} onclick={() => (draft!.mic_in = '')}>
          <span class="t-ico"><Icon name="refresh" size={18} /></span>
          <span class="t-txt"><strong>Automatic</strong><small>The Windows default microphone</small></span>
        </button>
        {#each captures.filter((d) => !isVirtual(d.name)) as d (d.id)}
          <button class="tile" class:on={draft.mic_in === d.id} class:bad={handsFree(d)} role="radio" aria-checked={draft.mic_in === d.id} onclick={() => (draft!.mic_in = d.id)}>
            <span class="t-ico"><Icon name="mic" size={18} /></span>
            <span class="t-txt"><strong>{shortName(d.name)}</strong><small>{handsFree(d) ? 'Hands-free: telephone quality' : `${d.sample_rate / 1000} kHz${d.default ? ' · Windows default' : ''}`}</small></span>
          </button>
        {/each}
      </div>
      <div class="row">
        <div>
          <strong>Pure microphone signal</strong>
          <p class="muted">Skips Windows' own noise suppression and auto-gain, which can make a voice sound robotic.</p>
        </div>
        <Switch checked={draft.mic_raw} label="Pure microphone signal" onchange={(v) => (draft!.mic_raw = v)} />
      </div>
    </section>

    {/if}

    <!-- the app itself -->
    {#if view === 'app' && desktop.available}
      <section class="panel card" aria-labelledby="app-h">
        <header>
          <span class="h-ico"><Icon name="system" size={18} /></span>
          <div>
            <h2 id="app-h">Audiora</h2>
            <p class="muted">Version {desktop.version}</p>
          </div>
        </header>
        <div class="row">
          <div>
            <strong>Start with Windows</strong>
            <p class="muted">Opens minimized when you sign in, so your channels and microphone are ready.</p>
          </div>
          <Switch checked={desktop.autostart} label="Start with Windows" onchange={(v) => void desktop.setAutostart(v)} />
        </div>
        <div class="row">
          <div>
            <strong>Updates</strong>
            <p class="muted">New versions are announced when Audiora opens and install with one click.</p>
          </div>
          <button class="btn sm" disabled={desktop.phase !== 'idle'} onclick={() => void desktop.check(true)}>{desktop.phase === 'checking' ? 'Checking…' : 'Check now'}</button>
        </div>
      </section>
    {/if}

    <!-- advanced routing -->
    {#if view === 'routing'}
    <section class="panel card" aria-labelledby="adv-h">
      <header>
        <span class="h-ico"><Icon name="routing" size={18} /></span>
        <div>
          <h2 id="adv-h">Routing</h2>
          <p class="muted">How apps reach the Game, Chat, Media, Aux and Music channels. Automatic is right for most setups.</p>
        </div>
      </header>

      {#if noCapture}
        <p class="note warn"><Icon name="alert" size={14} /> No output to capture apps through: Audiora needs one output you do not listen to (a digital/optical port or a monitor's HDMI audio).</p>
      {/if}

        <div class="adv">
          <div class="fields">
          <label class="field">
            <span>Capture apps through <small>an output you do not listen to</small></span>
            <select bind:value={draft.parking}>
              <option value="">Automatic{parked?.auto && parked.device_name ? ` · ${shortName(parked.device_name)}` : ''}</option>
              {#each parkable as d (d.id)}<option value={d.id}>{label(d)}</option>{/each}
            </select>
          </label>
          <label class="field">
            <span>Processed microphone to <small>a virtual cable (optional)</small></span>
            <select bind:value={draft.mic_out}>
              <option value="">Not sent anywhere</option>
              {#each renders as d (d.id)}<option value={d.id}>{label(d)}</option>{/each}
            </select>
          </label>
          </div>
          <div class="chans">
          <p class="sub">Virtual outputs per channel <small>(optional)</small></p>
          {#each APP_CHANNEL_KEYS as k (k)}
            {@const m = channelMeta(k)!}
            {@const st = chState(k)}
            {@const chosen = renders.find((d) => d.id === draft!.devices[k])}
            <div class="chrow" style:--c={m.color}>
              <span class="who"><i><Icon name={m.icon} size={13} /></i>{m.name}</span>
              <select bind:value={draft.devices[k]} aria-label="{m.name} output">
                <option value="">{st?.mode === 'direct' ? 'Direct capture' : 'Automatic'}{st?.auto && st.device_name ? ` · ${shortName(st.device_name)}` : ''}</option>
                {#each virtuals as d (d.id)}<option value={d.id} disabled={d.id === listening}>{label(d)}</option>{/each}
              </select>
              <span class="chip" class:ok={st?.online} class:warn={!st?.online}>{st?.mode === 'direct' ? 'direct' : st?.online ? 'ready' : 'not set'}</span>
            </div>
            {#if st?.error}<p class="note warn"><Icon name="alert" size={12} /> {st.error}</p>{/if}
            {#if chosen && crowded(chosen.name)}<p class="note warn"><Icon name="alert" size={12} /> {label(chosen)} belongs to another mixer: close it first or you may hear sound twice.</p>{/if}
          {/each}
          {#if duplicate}<p class="note warn"><Icon name="alert" size={12} /> Two channels (or the speakers) share a device. Each needs its own.</p>{/if}
          </div>
        </div>
    </section>
    {/if}
  </div>

  {#if dirty}
    <div class="apply" role="region" aria-label="Unsaved changes">
      <span><Icon name="alert" size={15} /> Changes take effect after a short audio restart.</span>
      <button class="btn sm ghost" onclick={discard} disabled={busy}>Discard</button>
      <button class="btn sm primary" onclick={apply} disabled={busy || duplicate}>{busy ? 'Applying…' : 'Apply changes'}</button>
    </div>
  {/if}
{/if}

<style>
  .settings {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
    align-items: start;
  }

  .settings.one {
    grid-template-columns: minmax(0, 1fr);
  }

  .card {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding: 18px 20px 20px;
    min-width: 0;
  }

  header {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .h-ico {
    flex: none;
    width: 38px;
    height: 38px;
    border-radius: 12px;
    display: grid;
    place-items: center;
    color: var(--accent-hi);
    background: color-mix(in oklab, var(--accent) 14%, transparent);
  }

  h2 {
    font-size: 16px;
    font-weight: 700;
    letter-spacing: -0.01em;
    color: var(--text);
    text-transform: none;
  }

  header p {
    font-size: 12.5px;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
    gap: 8px;
  }

  .tile {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 12px;
    border-radius: 14px;
    text-align: left;
    border: 1px solid var(--glass-line);
    background: rgba(255, 255, 255, 0.03);
    color: var(--text);
    transition: border-color 0.15s, background 0.15s, box-shadow 0.2s;
  }

  .tile:hover {
    background: rgba(255, 255, 255, 0.06);
  }

  .tile.on {
    border-color: color-mix(in oklab, var(--accent) 65%, transparent);
    background: color-mix(in oklab, var(--accent) 12%, transparent);
    box-shadow: 0 0 22px -8px var(--accent-glow);
  }

  .tile.bad small {
    color: var(--warn);
  }

  .t-ico {
    flex: none;
    width: 34px;
    height: 34px;
    border-radius: 10px;
    display: grid;
    place-items: center;
    background: var(--bg-2);
    color: var(--text-2);
  }

  .tile.on .t-ico {
    color: var(--accent-hi);
    background: color-mix(in oklab, var(--accent) 18%, transparent);
  }

  .t-txt {
    min-width: 0;
    display: flex;
    flex-direction: column;
  }

  .t-txt strong {
    font-size: 13px;
    font-weight: 650;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .t-txt small {
    font-size: 11.5px;
    color: var(--muted);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .row {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 12px 14px;
    border-radius: 14px;
    background: rgba(255, 255, 255, 0.03);
    border: 1px solid var(--glass-line);
  }

  .row > div {
    flex: 1;
    min-width: 0;
  }

  .row strong {
    font-size: 13px;
  }

  .row p {
    font-size: 12px;
  }

  /* the two device fields on the left, the per-channel outputs on the right */
  .adv {
    display: grid;
    grid-template-columns: minmax(0, 1fr) minmax(0, 1.3fr);
    gap: 24px;
    align-items: start;
  }

  .fields,
  .chans {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .field {
    display: grid;
    gap: 6px;
    font-size: 12px;
    color: var(--muted);
  }

  .sub {
    font-size: 12px;
    font-weight: 650;
    color: var(--text-2);
  }

  small {
    color: var(--faint);
    font-weight: 500;
  }

  .chrow {
    display: grid;
    grid-template-columns: 90px minmax(0, 1fr) auto;
    align-items: center;
    gap: 10px;
  }

  .chrow select,
  .field select {
    width: 100%;
    min-width: 0;
  }

  .who {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 650;
    font-size: 12.5px;
  }

  .who i {
    width: 22px;
    height: 22px;
    border-radius: 7px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: color-mix(in oklab, var(--c) 14%, transparent);
  }

  .note {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    font-size: 12px;
    margin: 0;
  }

  .note.warn {
    color: var(--warn);
  }

  .note :global(svg) {
    flex: none;
    margin-top: 2px;
  }

  /* floats above the page: nothing has to scroll to reach it */
  .apply {
    position: fixed;
    left: 50%;
    bottom: 44px;
    translate: -50% 0;
    width: min(720px, calc(100vw - 40px));
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 12px 14px 12px 18px;
    border-radius: 16px;
    background: color-mix(in oklab, var(--bg-2) 85%, transparent);
    border: 1px solid color-mix(in oklab, var(--accent) 40%, var(--glass-line));
    backdrop-filter: blur(18px);
    box-shadow: var(--shadow-pop);
    animation: rise 0.2s var(--ease);
  }

  .apply span {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    color: var(--text-2);
  }

  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(8px);
    }
  }

</style>
