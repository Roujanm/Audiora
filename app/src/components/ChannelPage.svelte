<script lang="ts">
  import { appColor, appLabel, channelMeta, SMART_LEVELS, smartLevelOf } from '../lib/channels.ts';
  import { engine, type AppGroup } from '../lib/engine.svelte.ts';
  import type { ChannelEntry } from '../lib/protocol.ts';
  import { shortName } from '../lib/theme.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import Card from './Card.svelte';
  import ChannelEq from './ChannelEq.svelte';
  import DynamicsPanel from './DynamicsPanel.svelte';
  import Icon from './Icon.svelte';
  import LedMeter from './LedMeter.svelte';
  import PresetBar from './PresetBar.svelte';
  import Segmented from './Segmented.svelte';
  import Slider from './Slider.svelte';
  import Switch from './Switch.svelte';

  interface Props {
    ch: ChannelEntry;
    onsetup: () => void;
  }
  let { ch, onsetup }: Props = $props();

  const meta = $derived(channelMeta(ch.key)!);
  const isMic = $derived(ch.kind === 'mic');
  const p = (leaf: string) => `strip/${ch.strip}/${leaf}`;
  const g = (leaf: string, d = 0) => engine.get(p(leaf), d);
  const on = (leaf: string) => g(leaf) > 0.5;

  const masterRate = $derived(engine.info?.buses[0]?.sample_rate ?? 48000);
  const clearCastOk = $derived(masterRate === 48000);
  const smartOn = $derived(on('smart/enabled'));
  const smartGain = $derived(smartOn ? engine.gauge(p('smart/gain_db')) : 0);
  const boost = $derived(g('boost_db'));
  const apps = $derived(engine.appGroups.filter((a) => a.route === ch.key));
  const profiles = $derived(
    Object.entries(engine.profiles)
      .filter(([, per]) => per[ch.key])
      .map(([app, per]) => ({ app, preset: per[ch.key] })),
  );
  const presetNames = $derived.by(() => {
    const l = engine.chPresets[ch.key];
    return l ? [...l.factory, ...l.user] : [];
  });
  const unprofiled = $derived(
    engine.appGroups.filter((a) => !profiles.some((p2) => p2.app === a.name.toLowerCase())),
  );
  const micMonitor = $derived((g('bus_mask') & 1) === 1);
  const micOut = $derived(engine.channels?.mic_out);
  const dbSigned = (v: number) => `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toFixed(1)} dB`;

  let addApp = $state('');

  async function guard(fn: () => Promise<unknown>) {
    try {
      await fn();
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  function setAppVolume(a: AppGroup, volume: number) {
    const onChannel = a.sessions.filter((s) => s.playing_in === ch.key);
    for (const s of onChannel.length ? onChannel : a.sessions) engine.setApp(s, { volume });
  }

  function setAppMute(a: AppGroup, mute: boolean) {
    const onChannel = a.sessions.filter((s) => s.playing_in === ch.key);
    for (const s of onChannel.length ? onChannel : a.sessions) engine.setApp(s, { mute });
  }
</script>

<div class="page" style:--c={meta.color}>
  <header class="hero">
    <span class="big"><Icon name={meta.icon} size={26} /></span>
    <div class="txt">
      <h2>{meta.name}</h2>
      <p class="muted">{meta.blurb}</p>
    </div>
    <div class="chips">
      {#if isMic}
        <span class="chip" class:ok={ch.online} class:bad={!ch.online}>{ch.online ? shortName(ch.device_name ?? 'Microphone') : 'No microphone'}</span>
      {:else if ch.online}
        <span class="chip ok" title={ch.device_name ?? ''}>{shortName(ch.device_name ?? 'Virtual device')}{ch.auto ? ' · auto' : ''}</span>
      {:else}
        <button class="chip warn setup" onclick={onsetup}><Icon name="alert" size={12} /> No virtual device — set up</button>
      {/if}
    </div>
  </header>

  <PresetBar channel={ch.key} color={meta.color} />

  {#if !isMic}
    <Card title="Apps in {meta.name}" icon="apps" color={meta.color} status={apps.length ? `${apps.length}` : ''} description="Drag apps here from the mixer. Volume here is per app, before the channel fader.">
      {#if apps.length}
        <ul class="apps">
          {#each apps as a (a.name)}
            <li class:idle={a.sessions.length === 0}>
              <span class="av" style:--a={appColor(a.name)}>{appLabel(a.name).charAt(0)}</span>
              <span class="nm">{appLabel(a.name)}{#if a.sessions.length === 0}<small>not running</small>{/if}</span>
              {#if a.sessions.length}
                <div class="vol">
                  <input
                    type="range"
                    min="0"
                    max="1"
                    step="0.01"
                    value={a.volume}
                    style="--fill: {a.volume * 100}%"
                    aria-label="{appLabel(a.name)} volume"
                    oninput={(e) => setAppVolume(a, +e.currentTarget.value)}
                  />
                  <output class="num">{Math.round(a.volume * 100)}</output>
                </div>
                <button class="mini" class:on={a.muted} aria-pressed={a.muted} title={a.muted ? 'Unmute' : 'Mute'} aria-label="Mute {appLabel(a.name)}" onclick={() => setAppMute(a, !a.muted)}>
                  <Icon name={a.muted ? 'x' : 'volume'} size={14} />
                </button>
              {:else}
                <span class="grow"></span>
              {/if}
              <button class="mini" title="Remove from {meta.name}" aria-label="Remove {appLabel(a.name)} from {meta.name}" onclick={() => guard(() => engine.routeApp(a.name, null))}>
                <Icon name="trash" size={14} />
              </button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="empty muted">Nothing is routed to {meta.name} yet. Drag an app from the mixer onto the {meta.name} column.</p>
      {/if}
    </Card>
  {/if}

  {#if isMic}
    <div class="two">
      <Card title="ClearCast noise cancellation" icon="bolt" color={meta.color} enabled={on('denoise')} onenabled={(v) => engine.set(p('denoise'), v)} description={clearCastOk ? 'AI that removes keyboards, fans and room noise from your voice (adds 10 ms).' : `Needs a 48 kHz master output; it runs at ${masterRate / 1000} kHz now.`}>
        <p class="note muted">{on('denoise') ? 'Active — your voice is cleaned in real time.' : 'Off — your microphone passes through untouched.'}</p>
      </Card>

      <Card title="Hear yourself" icon="headphones" color={meta.color} enabled={micMonitor} onenabled={(v) => engine.set(p('bus_mask'), (g('bus_mask') & ~1) | (v ? 1 : 0))} description="Sends your voice to the headphones so you can check the sound. Use headphones — speakers will feed back.">
        <div class="testmeter">
          <LedMeter orientation="horizontal" thickness={10} levels={[engine.peakDb(p('peak_l')), engine.peakDb(p('peak_r'))]} holds={[engine.holdDb(p('peak_l')), engine.holdDb(p('peak_r'))]} />
        </div>
        <p class="note muted">
          {#if micOut?.device_name}Processed voice goes to <strong>{shortName(micOut.device_name)}</strong> for other programs to use as a microphone.{:else}Other programs hear your microphone only through a virtual cable — choose a <button class="link" onclick={onsetup}>microphone output</button> in Setup.{/if}
        </p>
      </Card>
    </div>
  {:else if ch.key === 'chat'}
    <Card title="ClearCast noise cancellation" icon="bolt" color={meta.color} enabled={on('denoise')} onenabled={(v) => engine.set(p('denoise'), v)} description={clearCastOk ? 'Cleans up noisy voices from other people in the call.' : `Needs a 48 kHz master output; it runs at ${masterRate / 1000} kHz now.`}>
      <p class="note muted">{on('denoise') ? 'Active on everything played in Chat.' : 'Off.'}</p>
    </Card>
  {/if}

  <ChannelEq strip={ch.strip} color={meta.color} />

  <div class="two">
    <Card title="Volume boost" icon="volume" color={meta.color} status={boost > 0.05 ? dbSigned(boost) : 'Off'} description="Adds up to 12 dB ahead of the processing for sources that are too quiet.">
      <Slider label="Boost" value={boost} min={0} max={12} step={0.5} unit=" dB" reset={0} onchange={(v) => engine.set(p('boost_db'), v)} />
    </Card>

    <Card title="Smart Volume" icon="wave" color={meta.color} enabled={smartOn} onenabled={(v) => engine.set(p('smart/enabled'), v)} status={smartOn ? dbSigned(smartGain) : ''} description="Keeps loud moments from jumping out and quiet ones from disappearing.">
      <Segmented
        label="Smart Volume loudness"
        value={smartLevelOf(g('smart/target_db', -24))}
        options={SMART_LEVELS.map((l) => ({ id: l.id, label: l.label }))}
        onchange={(id) => engine.set(p('smart/target_db'), SMART_LEVELS.find((l) => l.id === id)!.db)}
      />
    </Card>
  </div>

  <section class="dyn" aria-label="Gate and compressor">
    <h3 class="eyebrow">Dynamics</h3>
    <DynamicsPanel strip={ch.strip} showNoise={false} />
  </section>

  {#if !isMic}
    <Card title="Game profiles" icon="star" color={meta.color} description="Pick a preset for a program and it is applied by itself whenever that program starts.">
      {#if profiles.length}
        <ul class="prof">
          {#each profiles as pr (pr.app)}
            <li>
              <span class="av" style:--a={appColor(pr.app)}>{appLabel(pr.app).charAt(0)}</span>
              <span class="nm">{appLabel(pr.app)}</span>
              <select aria-label="Preset for {pr.app}" value={pr.preset} onchange={(e) => guard(() => engine.setProfile(pr.app, ch.key, e.currentTarget.value))}>
                {#each presetNames as n (n)}<option value={n}>{n}</option>{/each}
              </select>
              <button class="mini" title="Remove" aria-label="Remove profile for {pr.app}" onclick={() => guard(() => engine.setProfile(pr.app, ch.key, null))}><Icon name="trash" size={14} /></button>
            </li>
          {/each}
        </ul>
      {/if}
      <div class="add">
        <select aria-label="Program" bind:value={addApp}>
          <option value="">Choose a program…</option>
          {#each unprofiled as a (a.name)}<option value={a.name.toLowerCase()}>{appLabel(a.name)}</option>{/each}
        </select>
        <button
          class="btn sm"
          disabled={!addApp}
          onclick={() =>
            guard(async () => {
              await engine.setProfile(addApp, ch.key, engine.activePreset[ch.key] ?? 'Flat');
              addApp = '';
            })}
        ><Icon name="plus" size={13} /> Add profile</button>
      </div>
      {#if !profiles.length}
        <p class="note muted">Example: give “Call of Duty” the Footsteps preset on Game and it turns itself on when the game opens.</p>
      {/if}
    </Card>
  {/if}
</div>

<style>
  .page {
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-bottom: 8px;
    /* switches, sliders and primary buttons on this page wear the channel's colour */
    --accent: var(--c);
    --accent-hi: color-mix(in oklab, var(--c) 70%, white);
    --accent-lo: color-mix(in oklab, var(--c) 60%, black);
    --on-accent: #06101c;
    --accent-soft: color-mix(in oklab, var(--c) 16%, transparent);
    --accent-glow: color-mix(in oklab, var(--c) 50%, transparent);
  }

  .hero {
    display: flex;
    align-items: center;
    gap: 16px;
  }

  .big {
    flex: none;
    width: 52px;
    height: 52px;
    border-radius: 16px;
    display: grid;
    place-items: center;
    color: var(--c);
    background: color-mix(in oklab, var(--c) 14%, transparent);
    border: 1px solid color-mix(in oklab, var(--c) 32%, transparent);
    box-shadow: 0 10px 30px -12px var(--c);
  }

  .txt {
    flex: 1;
    min-width: 0;
  }

  h2 {
    font-size: 22px;
    font-weight: 750;
    letter-spacing: -0.02em;
  }

  .chips {
    flex: none;
  }

  .setup {
    cursor: pointer;
  }

  .two {
    display: grid;
    grid-template-columns: repeat(2, minmax(0, 1fr));
    gap: 14px;
    align-items: start;
  }

  .note {
    font-size: 12px;
  }

  .note strong {
    color: var(--text-2);
    font-weight: 600;
  }

  .link {
    border: 0;
    background: none;
    padding: 0;
    color: var(--accent-hi);
    text-decoration: underline;
  }

  .testmeter {
    margin-bottom: 10px;
  }

  .dyn {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .dyn h3 {
    padding-left: 4px;
  }

  .apps,
  .prof {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .apps li,
  .prof li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px 6px 6px;
    border-radius: var(--r-md);
    background: var(--bg-2);
    border: 1px solid var(--line);
  }

  .apps li.idle {
    opacity: 0.6;
  }

  .av {
    flex: none;
    width: 30px;
    height: 30px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    font-weight: 750;
    color: #0b0e13;
    background: linear-gradient(135deg, var(--a), color-mix(in oklab, var(--a) 60%, #000));
  }

  .nm {
    width: 150px;
    flex: none;
    font-weight: 600;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .nm small {
    font-weight: 500;
    font-size: 10.5px;
    color: var(--faint);
  }

  .vol {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
  }

  .vol output {
    width: 28px;
    text-align: right;
    font-size: 12px;
    color: var(--muted);
  }

  .grow {
    flex: 1;
  }

  .mini {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    padding: 0;
    border: 1px solid transparent;
    background: none;
    color: var(--faint);
  }

  .mini:hover {
    color: var(--text);
    background: var(--bg-4);
  }

  .mini.on {
    color: var(--bad);
  }

  .empty {
    font-size: 12.5px;
  }

  .prof select {
    flex: 1;
    min-width: 0;
  }

  .add {
    display: flex;
    gap: 8px;
    margin-top: 10px;
  }

  .add select {
    flex: 1;
    max-width: 320px;
  }

  .prof + .add {
    margin-top: 12px;
  }

  @media (max-width: 1100px) {
    .two {
      grid-template-columns: 1fr;
    }
  }
</style>
