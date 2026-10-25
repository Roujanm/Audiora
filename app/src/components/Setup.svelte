<script lang="ts">
  import { APP_CHANNEL_KEYS, channelMeta } from '../lib/channels.ts';
  import { engine } from '../lib/engine.svelte.ts';
  import { shortName } from '../lib/theme.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Icon from './Icon.svelte';

  const cfg = $derived(engine.channels?.config);
  const renders = $derived(engine.devices.filter((d) => d.flow === 'render'));
  const captures = $derived(engine.devices.filter((d) => d.flow === 'capture'));

  type Draft = { master_out: string; mic_in: string; mic_out: string; devices: Record<string, string> };
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
      APP_CHANNEL_KEYS.some((k) => draft!.devices[k] !== (cfg.devices[k] ?? ''))
    );
  });

  const crowded = (name: string) => /sonar|steelseries|voicemeeter/i.test(name);
  const picked = $derived(draft ? APP_CHANNEL_KEYS.map((k) => draft!.devices[k]).filter(Boolean) : []);
  const duplicate = $derived(new Set(picked).size !== picked.length || (!!draft?.master_out && picked.includes(draft.master_out)));

  async function apply() {
    if (!draft) return;
    busy = true;
    try {
      await engine.configSet({
        master_out: draft.master_out || null,
        mic_in: draft.mic_in || null,
        mic_out: draft.mic_out || null,
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
  const label = (d: { name: string; flow: string; sample_rate: number }) =>
    shortName(d.name) + (d.flow === 'capture' && d.sample_rate > 0 && d.sample_rate < 16000 ? ' — hands-free (makes the headset sound robotic)' : '');
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
            <option value="">Automatic{engine.channels?.master_out.auto && engine.channels.master_out.device_name ? ` — ${shortName(engine.channels.master_out.device_name)}` : ''}</option>
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
        <p class="muted tiny">
          For other programs to use your cleaned-up voice, pick a cable here and choose that cable's <em>output</em> as the microphone in them.
        </p>
      </div>

      <div class="block">
        <h3>Virtual outputs</h3>
        {#each APP_CHANNEL_KEYS as k (k)}
          {@const m = channelMeta(k)!}
          {@const st = chState(k)}
          <label style:--c={m.color}>
            <span class="who"><i><Icon name={m.icon} size={13} /></i>{m.name}</span>
            <select bind:value={draft.devices[k]}>
              <option value="">Automatic{st?.auto && st.device_name ? ` — ${shortName(st.device_name)}` : ''}</option>
              {#each renders as d (d.id)}<option value={d.id}>{label(d)}</option>{/each}
            </select>
            <span class="chip" class:ok={st?.online} class:warn={!st?.online}>{st?.online ? 'ready' : 'no device'}</span>
          </label>
          {@const chosen = renders.find((d) => d.id === draft!.devices[k])}
          {#if chosen && crowded(chosen.name)}
            <p class="warn-line"><Icon name="alert" size={12} /> {label(chosen)} belongs to another mixer — close it first or you may hear sound twice.</p>
          {/if}
        {/each}
        {#if duplicate}<p class="warn-line"><Icon name="alert" size={12} /> Two channels (or the speakers) share a device. Each needs its own.</p>{/if}
      </div>
    </div>

    {#if !engine.channels?.channels.some((c) => c.kind === 'apps' && c.online)}
      <div class="howto">
        <Icon name="alert" size={16} />
        <div>
          <strong>No virtual outputs found.</strong> Install virtual audio cables (for example the free <em>VB-Audio Virtual Cable</em> pack — A, B, C and D give you four),
          then pick one for each channel above. Audiora's own driver will replace this step in a later release.
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
