<script lang="ts">
  import { APP_CHANNEL_KEYS, channelMeta } from '../lib/channels.ts';
  import { desktop } from '../lib/desktop.svelte.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import { shortName } from '../lib/theme.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';
  import Switch from './Switch.svelte';

  const cfg = $derived(engine.channels?.config);
  const renders = $derived(engine.devices.filter((d) => d.flow === 'render'));
  const captures = $derived(engine.devices.filter((d) => d.flow === 'capture'));

  type Draft = { master_out: string; mic_in: string; mic_out: string; parking: string; mic_raw: boolean; sim_r: boolean; devices: Record<string, string> };
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
      sim_r: c.sim_r ?? false,
      devices: Object.fromEntries(APP_CHANNEL_KEYS.map((k) => [k, c.devices[k] ?? ''])),
    };
  });

  $effect(() => {
    void engine.refreshDevices().catch(() => {});
  });

  const dirty = $derived.by(() => {
    if (!draft || !cfg) return false;
    return (
      draft.master_out !== (cfg.master_out ?? '') ||
      draft.mic_in !== (cfg.mic_in ?? '') ||
      draft.mic_out !== (cfg.mic_out ?? '') ||
      draft.parking !== (cfg.parking ?? '') ||
      draft.mic_raw !== (cfg.mic_raw ?? true) ||
      draft.sim_r !== (cfg.sim_r ?? false) ||
      APP_CHANNEL_KEYS.some((k) => draft!.devices[k] !== (cfg.devices[k] ?? ''))
    );
  });

  const crowded = (name: string) => /sonar|steelseries|voicemeeter/i.test(name);
  // Same list as the engine: only virtual outputs can be channels. A real speaker as a channel
  // would record what it plays and send it back into itself (endless echo).
  const isVirtual = (name: string) => /sonar|voicemeeter|vb-audio|cable|virtual|audiora|steelseries|hi-fi/i.test(name);
  const virtuals = $derived(renders.filter((d) => isVirtual(d.name)));
  const listening = $derived(draft?.master_out || engine.channels?.master_out.device_id || '');
  const picked = $derived(draft ? APP_CHANNEL_KEYS.map((k) => draft!.devices[k]).filter(Boolean) : []);
  // outputs that can park apps: real outputs that are neither the speakers nor a channel's device
  const parkable = $derived(renders.filter((d) => !isVirtual(d.name) && d.id !== listening));
  const parked = $derived(engine.channels?.parking);
  const duplicate = $derived(new Set(picked).size !== picked.length || (!!listening && picked.includes(listening)));

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
        sim_r: draft.sim_r,
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

  // Bluetooth hands-free microphones run at 8-16 kHz and switch the headset to telephone quality
  const label = (d: { name: string; flow: string; sample_rate: number; default?: boolean }) =>
    shortName(d.name) +
    (d.default ? ' · Windows default' : '') +
    (d.flow === 'capture' && d.sample_rate > 0 && d.sample_rate < 16000 ? ' — hands-free (makes the headset sound robotic)' : '');
  const chState = (k: string) => engine.channels?.channels.find((c) => c.key === k);
</script>

{#if draft}
  <section class="panel pad setup" aria-label="Channel setup">
    <header>
      <div>
        <h2 class="eyebrow">Channels &amp; devices</h2>
        <p class="muted">
          Each app channel listens to one <strong>virtual output</strong>. Windows is told to play the apps you drag into that channel
          there, and Audiora mixes them to your speakers — no driver of ours needed yet.
        </p>
      </div>
      <button class="btn primary" disabled={!dirty || duplicate || busy} onclick={apply}>
        <Icon name="refresh" size={14} /> {busy ? 'Applying…' : 'Save & restart audio'}
      </button>
    </header>

    <div class="grid">
      <div class="block">
        <h3>Where you listen</h3>
        <label>
          <span>Speakers / headphones</span>
          <select bind:value={draft.master_out}>
            <option value="">Automatic, follows Windows{engine.channels?.master_out.auto && engine.channels.master_out.device_name ? ` — now ${shortName(engine.channels.master_out.device_name)}` : ''}</option>
            {#each renders as d (d.id)}<option value={d.id}>{label(d)}</option>{/each}
          </select>
        </label>
        <label>
          <span>Microphone</span>
          <select bind:value={draft.mic_in}>
            <option value="">Automatic</option>
            {#each captures as d (d.id)}<option value={d.id}>{label(d)}</option>{/each}
          </select>
        </label>
        <label>
          <span>Processed microphone → <small>virtual cable input (optional)</small></span>
          <select bind:value={draft.mic_out}>
            <option value="">Not sent anywhere</option>
            {#each renders as d (d.id)}<option value={d.id}>{label(d)}</option>{/each}
          </select>
        </label>
        <div class="toggle">
          <div>
            <strong>Pure microphone signal</strong>
            <p class="muted tiny">Skips Windows and driver "enhancements" (their noise suppression and auto-gain can make a voice sound robotic). Audiora's own processing does the work.</p>
          </div>
          <Switch checked={draft.mic_raw} label="Pure microphone signal" onchange={(v) => (draft!.mic_raw = v)} />
        </div>
        {#if desktop.available}
          <div class="toggle">
            <div>
              <strong>Start with Windows</strong>
              <p class="muted tiny">Audiora opens minimized when you sign in, so your channels and microphone are ready.</p>
            </div>
            <Switch checked={desktop.autostart} label="Start with Windows" onchange={(v) => void desktop.setAutostart(v)} />
          </div>
          <div class="toggle">
            <div>
              <strong>Updates</strong>
              <p class="muted tiny">Version {desktop.version}. New versions are announced in the app and install with one click.</p>
            </div>
            <button class="btn sm" disabled={desktop.phase !== 'idle'} onclick={() => void desktop.check(true)}>{desktop.phase === 'checking' ? 'Checking…' : 'Check now'}</button>
          </div>
        {/if}
        <div class="toggle">
          <div>
            <strong>R Interface simulator</strong>
            <p class="muted tiny">For development without the hardware: a simulated R Interface appears on the Interface page.</p>
          </div>
          <Switch checked={draft.sim_r} label="R Interface simulator" onchange={(v) => (draft!.sim_r = v)} />
        </div>
        <p class="muted tiny">
          For other programs to use your cleaned-up voice, pick a cable here and choose that cable's <em>output</em> as the microphone in them.
        </p>
      </div>

      <div class="block">
        <h3>How apps reach the channels</h3>
        <label>
          <span>Direct capture through <small>an output you do not listen to</small></span>
          <select bind:value={draft.parking}>
            <option value="">Automatic{parked?.auto && parked.device_name ? ` — ${shortName(parked.device_name)}` : ''}</option>
            {#each parkable as d (d.id)}<option value={d.id}>{label(d)}</option>{/each}
          </select>
        </label>
        <p class="muted tiny">
          Routed apps play into this output while Audiora takes their sound straight from Windows — no driver or cable
          needed. Choose a port with nothing plugged in (digital/optical or a monitor without speakers), otherwise you
          hear those apps twice.
        </p>

        <h3 class="sub">Virtual outputs <small>(optional)</small></h3>
        {#each APP_CHANNEL_KEYS as k (k)}
          {@const m = channelMeta(k)!}
          {@const st = chState(k)}
          <label style:--c={m.color}>
            <span class="who"><i><Icon name={m.icon} size={13} /></i>{m.name}</span>
            <select bind:value={draft.devices[k]}>
              <option value="">{st?.mode === 'direct' ? 'Direct capture' : 'Automatic'}{st?.auto && st.device_name ? ` — ${shortName(st.device_name)}` : ''}</option>
              {#each virtuals as d (d.id)}<option value={d.id} disabled={d.id === listening}>{label(d)}</option>{/each}
            </select>
            <span class="chip" class:ok={st?.online} class:warn={!st?.online}>{st?.mode === 'direct' ? 'direct' : st?.online ? 'ready' : 'not set'}</span>
          </label>
          {#if st?.error}<p class="warn-line"><Icon name="alert" size={12} /> {st.error}</p>{/if}
          {@const chosen = renders.find((d) => d.id === draft!.devices[k])}
          {#if chosen && crowded(chosen.name)}
            <p class="warn-line"><Icon name="alert" size={12} /> {label(chosen)} belongs to another mixer — close it first or you may hear sound twice.</p>
          {/if}
        {/each}
        {#if duplicate}<p class="warn-line"><Icon name="alert" size={12} /> Two channels (or the speakers) share a device. Each needs its own.</p>{/if}
        <p class="muted tiny">Only virtual outputs are listed: a real speaker here would echo its own sound back forever.</p>
      </div>
    </div>

    {#if !engine.channels?.channels.some((c) => c.kind === 'apps' && c.online)}
      <div class="howto">
        <Icon name="alert" size={16} />
        <div>
          <strong>No output to capture apps through.</strong> Direct capture needs one output you do not listen to (a digital/optical
          port or a monitor's HDMI audio). Choose one above, or plug in any second audio device.
        </div>
      </div>
    {/if}
  </section>
{/if}

<style>
  .setup {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  header {
    display: flex;
    justify-content: space-between;
    align-items: flex-start;
    gap: 20px;
  }

  header p {
    margin-top: 4px;
    max-width: 640px;
    font-size: 12.5px;
  }

  header strong {
    color: var(--text-2);
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 22px;
  }

  .block {
    display: flex;
    flex-direction: column;
    gap: 10px;
    min-width: 0;
  }

  h3.sub {
    margin-top: 8px;
  }

  h3 small {
    font-weight: 500;
    color: var(--faint);
  }

  h3 {
    font-size: 12px;
    font-weight: 650;
    color: var(--text-2);
  }

  label {
    display: grid;
    gap: 5px;
    font-size: 12px;
    color: var(--muted);
  }

  .block:last-child label {
    grid-template-columns: 92px 1fr auto;
    align-items: center;
    gap: 10px;
  }

  label select {
    width: 100%;
    min-width: 0;
  }

  .who {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 650;
    color: var(--text);
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

  small {
    color: var(--faint);
  }

  .toggle {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 10px 12px;
    border-radius: var(--r-md);
    background: var(--bg-2);
  }

  .toggle strong {
    font-size: 12.5px;
    color: var(--text);
  }

  .tiny {
    font-size: 11.5px;
  }

  .warn-line {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    color: var(--warn);
    margin: -4px 0 0;
  }

  .howto {
    display: flex;
    gap: 12px;
    padding: 12px 14px;
    border-radius: var(--r-md);
    background: color-mix(in oklab, var(--warn) 9%, transparent);
    border: 1px solid color-mix(in oklab, var(--warn) 30%, transparent);
    color: var(--text-2);
    font-size: 12.5px;
  }

  .howto :global(svg) {
    flex: none;
    color: var(--warn);
    margin-top: 1px;
  }

  @media (max-width: 1000px) {
    .grid {
      grid-template-columns: 1fr;
    }
  }
</style>
