<script lang="ts">
  import { appColor, appLabel, channelMeta, SMART_LEVELS, smartLevelOf } from '../lib/channels.ts';
  import { engine, type AppGroup } from '../lib/engine.svelte.ts';
  import type { ChannelEntry } from '../lib/protocol.ts';
  import { shortName } from '../lib/theme.ts';
  import { toast } from '../lib/toast.svelte.ts';
  import ChannelEq from './ChannelEq.svelte';
  import DynamicsPanel from './DynamicsPanel.svelte';
  import MicCheck from './MicCheck.svelte';
  import Icon from './Icon.svelte';
  import Popover from './Popover.svelte';
  import PresetPicker from './PresetPicker.svelte';
  import Segmented from './Segmented.svelte';
  import SettingRow from './SettingRow.svelte';

  interface Props {
    ch: ChannelEntry;
    onsetup: () => void;
  }
  let { ch, onsetup }: Props = $props();

  const meta = $derived(channelMeta(ch.key)!);
  const isMic = $derived(ch.kind === 'mic');
  const boostMax = $derived(isMic ? 48 : 12);
  const p = (leaf: string) => `strip/${ch.strip}/${leaf}`;
  const g = (leaf: string, d = 0) => engine.get(p(leaf), d);
  const on = (leaf: string) => g(leaf) > 0.5;

  const clearCastOk = $derived((engine.info?.buses[0]?.sample_rate ?? 48000) === 48000);
  const boost = $derived(g('boost_db'));
  const smartGain = $derived(on('smart/enabled') ? engine.gauge(p('smart/gain_db')) : 0);
  const gr = $derived(on('comp/enabled') ? engine.gauge(p('comp/gr_db')) : 0);
  const gateOpen = $derived(engine.gauge(p('gate/open')) > 0.5);
  const monitoring = $derived((g('bus_mask') & 1) === 1);
  const micOut = $derived(engine.channels?.mic_out);
  const lists = $derived(engine.chPresets[ch.key] ?? { factory: [], user: [], favorites: [] });
  const current = $derived(engine.activePreset[ch.key] ?? '');
  const apps = $derived(engine.appGroups.filter((a) => a.route === ch.key));
  const profiles = $derived(
    Object.entries(engine.profiles)
      .filter(([, per]) => per[ch.key])
      .map(([app, per]) => ({ app, preset: per[ch.key] })),
  );
  const presetNames = $derived([...lists.factory, ...lists.user]);
  const unprofiled = $derived(engine.appGroups.filter((a) => !profiles.some((x) => x.app === a.name.toLowerCase())));

  // compressor "amount" 0..100 drives threshold, ratio and make-up together
  const amount = $derived(Math.round(Math.min(100, Math.max(0, (-6 - g('comp/threshold_db', -18)) / 0.3))));
  function setAmount(a: number) {
    engine.set(p('comp/threshold_db'), Math.round((-6 - a * 0.3) * 10) / 10);
    engine.set(p('comp/ratio'), Math.round((2 + a * 0.04) * 10) / 10);
    engine.set(p('comp/makeup_db'), Math.round(a * 0.08 * 10) / 10);
  }

  // de-esser "amount" 0..100 maps to its threshold: more amount = acts on softer sibilance
  const deessAmount = $derived(Math.round(Math.min(100, Math.max(0, (-15 - g('deess/threshold_db', -30)) / 0.35))));
  const deessGr = $derived(on('deess/enabled') ? engine.gauge(p('deess/gr_db')) : 0);
  const limitGr = $derived(on('limiter/enabled') ? engine.gauge(p('limiter/gr_db')) : 0);
  const LOWCUTS = [60, 80, 100, 120];

  const signed = (v: number) => `${v > 0 ? '+' : v < 0 ? '−' : ''}${Math.abs(v).toFixed(1)} dB`;
  let saveName = $state('');
  let addApp = $state('');
  let advanced = $state(false);

  async function guard(fn: () => Promise<unknown>, ok?: string) {
    try {
      await fn();
      if (ok) toast(ok, 'ok');
    } catch (e) {
      toast(String(e instanceof Error ? e.message : e), 'error');
    }
  }

  function sessionsOf(a: AppGroup) {
    const here = a.sessions.filter((s) => s.playing_in === ch.key);
    return here.length ? here : a.sessions;
  }
</script>

<div class="page" style:--c={meta.color}>
  <div class="bar">
    <div class="favs" role="group" aria-label="Favorite presets">
      {#if !ch.online && !isMic}
        <span class="warn">This channel has no virtual output yet — <button class="link" onclick={onsetup}>set it up</button>.</span>
      {:else}
        {#each lists.favorites as f (f)}
          <button class="fav" class:on={f === current} aria-pressed={f === current} onclick={() => guard(() => engine.loadChannelPreset(ch.key, f))}>
            <Icon name="star" size={11} />{f}
          </button>
        {:else}
          <span class="muted hint">Star presets to keep them one click away.</span>
        {/each}
      {/if}
    </div>
    <PresetPicker channel={ch.key} color={meta.color} />
    <Popover width={260}>
      {#snippet trigger({ open, toggle })}
        <button class="btn icon-btn" class:open title="Save as a new preset" aria-label="Save as a new preset" onclick={toggle}><Icon name="save" size={16} /></button>
      {/snippet}
      {#snippet children({ close })}
        <form
          class="save"
          onsubmit={(e) => {
            e.preventDefault();
            const n = saveName.trim();
            if (!n) return;
            void guard(async () => {
              await engine.saveChannelPreset(ch.key, n);
              saveName = '';
              close();
            }, `Saved “${n}”`);
          }}
        >
          <p class="eyebrow">Save as preset</p>
          <input type="text" bind:value={saveName} placeholder="Name" maxlength="40" aria-label="Preset name" />
          <button class="btn sm primary" type="submit" disabled={!saveName.trim()}>Save</button>
        </form>
      {/snippet}
    </Popover>
  </div>

  {#if isMic}<MicCheck />{/if}

  <ChannelEq strip={ch.strip} color={meta.color} />

  <section class="panel list" aria-label="Sound settings">
    {#if isMic}
      <SettingRow icon="wave" title="Low cut" description="Removes rumble, desk bumps and breath pops." on={on('lowcut/enabled')} ontoggle={(v) => engine.set(p('lowcut/enabled'), v)}>
        <Segmented
          size="sm"
          label="Low cut frequency"
          value={String(LOWCUTS.reduce((b, f) => (Math.abs(f - g('lowcut/freq', 80)) < Math.abs(b - g('lowcut/freq', 80)) ? f : b)))}
          options={LOWCUTS.map((f) => ({ id: String(f), label: `${f} Hz` }))}
          onchange={(id) => engine.set(p('lowcut/freq'), +id)}
        />
      </SettingRow>
    {/if}

    {#if isMic || ch.key === 'chat'}
      <SettingRow
        icon="bolt"
        title="ClearCast noise cancellation"
        description={clearCastOk ? (isMic ? 'AI that removes keyboard, fan and room noise from your voice.' : 'Cleans up noisy voices from the people you talk to.') : 'Needs the speakers to run at 48 kHz.'}
        on={on('denoise')}
        disabled={!clearCastOk}
        ontoggle={(v) => engine.set(p('denoise'), v)}
      >
        <input
          type="range"
          min="0"
          max="1"
          step="0.05"
          value={g('denoise_mix', 1)}
          style="--fill: {g('denoise_mix', 1) * 100}%"
          aria-label="Noise cancellation strength"
          oninput={(e) => engine.set(p('denoise_mix'), +e.currentTarget.value)}
        />
        <output class="num">{Math.round(g('denoise_mix', 1) * 100)}%</output>
      </SettingRow>
    {/if}

    {#if isMic}
      <SettingRow icon="mic" title="Noise gate" description="Silences the microphone between words." on={on('gate/enabled')} ontoggle={(v) => engine.set(p('gate/enabled'), v)}>
        <span class="led" class:open={gateOpen && on('gate/enabled')} title={gateOpen ? 'Open' : 'Closed'}></span>
        <input
          type="range"
          min="-80"
          max="-20"
          step="1"
          value={g('gate/threshold_db', -50)}
          style="--fill: {((g('gate/threshold_db', -50) + 80) / 60) * 100}%"
          aria-label="Gate threshold"
          oninput={(e) => engine.set(p('gate/threshold_db'), +e.currentTarget.value)}
        />
        <output class="num">{g('gate/threshold_db', -50).toFixed(0)} dB</output>
      </SettingRow>
    {/if}

    {#if isMic}
      <SettingRow icon="chat" title="De-esser" description="Softens harsh “s” and “sh” sounds." value={deessGr > 0.5 ? `−${deessGr.toFixed(1)} dB` : ''} on={on('deess/enabled')} ontoggle={(v) => engine.set(p('deess/enabled'), v)}>
        <input
          type="range"
          min="0"
          max="100"
          step="1"
          value={deessAmount}
          style="--fill: {deessAmount}%"
          aria-label="De-esser amount"
          oninput={(e) => engine.set(p('deess/threshold_db'), Math.round((-15 - +e.currentTarget.value * 0.35) * 10) / 10)}
        />
        <output class="num">{deessAmount}%</output>
      </SettingRow>
    {/if}

    <SettingRow
      icon="volume"
      title={isMic ? 'Mic gain' : 'Volume boost'}
      description={isMic ? 'Up to +48 dB for quiet dynamic mics. The limiter keeps it from clipping.' : 'For sources that are too quiet.'}
      value={boost > 0.05 ? signed(boost) : ''}
    >
      <input type="range" min="0" max={boostMax} step="0.5" value={boost} style="--fill: {(Math.min(boost, boostMax) / boostMax) * 100}%" aria-label={isMic ? 'Mic gain' : 'Volume boost'} oninput={(e) => engine.set(p('boost_db'), +e.currentTarget.value)} />
    </SettingRow>

    <SettingRow
      icon="wave"
      title="Smart Volume"
      description="Evens out loud and quiet moments."
      value={on('smart/enabled') && Math.abs(smartGain) > 0.4 ? signed(smartGain) : ''}
      on={on('smart/enabled')}
      ontoggle={(v) => engine.set(p('smart/enabled'), v)}
    >
      <Segmented
        size="sm"
        label="Smart Volume loudness"
        value={smartLevelOf(g('smart/target_db', -24))}
        options={SMART_LEVELS.map((l) => ({ id: l.id, label: l.label }))}
        onchange={(id) => engine.set(p('smart/target_db'), SMART_LEVELS.find((l) => l.id === id)!.db)}
      />
    </SettingRow>

    <SettingRow
      icon="sliders"
      title="Compressor"
      description={isMic ? 'Keeps your voice at a steady level.' : 'Tames sudden peaks.'}
      value={gr > 0.3 ? `−${gr.toFixed(1)} dB` : ''}
      on={on('comp/enabled')}
      ontoggle={(v) => engine.set(p('comp/enabled'), v)}
    >
      <input type="range" min="0" max="100" step="1" value={amount} style="--fill: {amount}%" aria-label="Compression amount" oninput={(e) => setAmount(+e.currentTarget.value)} />
      <output class="num">{amount}%</output>
    </SettingRow>

    {#if isMic}
      <SettingRow
        icon="alert"
        title="Limiter"
        description="Never lets your voice clip, however loud you get."
        value={limitGr > 0.3 ? `−${limitGr.toFixed(1)} dB` : ''}
        on={on('limiter/enabled')}
        ontoggle={(v) => engine.set(p('limiter/enabled'), v)}
      />
      <SettingRow
        icon="headphones"
        title="Hear yourself"
        description={micOut?.device_name ? `Your processed voice also goes to ${shortName(micOut.device_name)}.` : 'Use headphones — speakers will feed back.'}
        on={monitoring}
        ontoggle={(v) => engine.set(p('bus_mask'), (g('bus_mask') & ~1) | (v ? 1 : 0))}
      />
    {/if}
  </section>

  {#if !isMic}
    <section class="panel block" aria-label="Apps in {meta.name}">
      <h3>Apps in {meta.name}</h3>
      {#if apps.length}
        <ul class="rows">
          {#each apps as a (a.name)}
            <li class:idle={a.sessions.length === 0}>
              <span class="av" style:--a={appColor(a.name)}>{appLabel(a.name).charAt(0)}</span>
              <span class="nm">{appLabel(a.name)}</span>
              {#if a.sessions.length}
                <input
                  type="range"
                  min="0"
                  max="1"
                  step="0.01"
                  value={a.volume}
                  style="--fill: {a.volume * 100}%"
                  aria-label="{appLabel(a.name)} volume"
                  oninput={(e) => sessionsOf(a).forEach((s) => engine.setApp(s, { volume: +e.currentTarget.value }))}
                />
                <output class="num">{Math.round(a.volume * 100)}%</output>
              {:else}
                <span class="muted grow">Not running</span>
              {/if}
              <button class="mini" title="Remove from {meta.name}" aria-label="Remove {appLabel(a.name)}" onclick={() => guard(() => engine.routeApp(a.name, null))}><Icon name="x" size={14} /></button>
            </li>
          {/each}
        </ul>
      {:else}
        <p class="muted">Drag apps onto the {meta.name} column in the mixer.</p>
      {/if}

      <h3 class="sub">Game profiles <small>— load a preset by itself when a program starts</small></h3>
      {#if profiles.length}
        <ul class="rows">
          {#each profiles as pr (pr.app)}
            <li>
              <span class="av" style:--a={appColor(pr.app)}>{appLabel(pr.app).charAt(0)}</span>
              <span class="nm">{appLabel(pr.app)}</span>
              <select aria-label="Preset for {pr.app}" value={pr.preset} onchange={(e) => guard(() => engine.setProfile(pr.app, ch.key, e.currentTarget.value))}>
                {#each presetNames as n (n)}<option value={n}>{n}</option>{/each}
              </select>
              <button class="mini" title="Remove" aria-label="Remove profile for {pr.app}" onclick={() => guard(() => engine.setProfile(pr.app, ch.key, null))}><Icon name="x" size={14} /></button>
            </li>
          {/each}
        </ul>
      {/if}
      <div class="add">
        <select aria-label="Program" bind:value={addApp}>
          <option value="">Choose a program…</option>
          {#each unprofiled as a (a.name)}<option value={a.name.toLowerCase()}>{appLabel(a.name)}</option>{/each}
        </select>
        <button class="btn sm" disabled={!addApp} onclick={() => guard(async () => { await engine.setProfile(addApp, ch.key, current || 'Flat'); addApp = ''; })}>
          <Icon name="plus" size={13} /> Add
        </button>
      </div>
    </section>
  {/if}

  <section class="adv">
    <button class="disclose" aria-expanded={advanced} onclick={() => (advanced = !advanced)}>
      <span class="chev" class:open={advanced}><Icon name="arrow" size={14} /></span> Advanced: gate and compressor details
    </button>
    {#if advanced}<DynamicsPanel strip={ch.strip} showNoise={false} />{/if}
  </section>
</div>

<style>
  .page {
    max-width: 980px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    gap: 14px;
    padding-bottom: 12px;
    /* controls on this page wear the channel's colour */
    --accent: var(--c);
    --accent-hi: color-mix(in oklab, var(--c) 70%, white);
    --accent-lo: color-mix(in oklab, var(--c) 60%, black);
    --on-accent: #06101c;
    --accent-soft: color-mix(in oklab, var(--c) 15%, transparent);
    --accent-glow: color-mix(in oklab, var(--c) 50%, transparent);
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .warn {
    color: var(--warn);
    font-size: 12.5px;
  }

  .hint {
    font-size: 12px;
  }

  .link {
    border: 0;
    background: none;
    padding: 0;
    color: inherit;
    text-decoration: underline;
    font: inherit;
  }

  .icon-btn {
    width: 36px;
    height: 36px;
    padding: 0;
  }

  .save {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 4px;
  }

  .favs {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 6px;
  }

  .fav {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    height: 28px;
    padding: 0 12px;
    border-radius: 999px;
    border: 1px solid var(--line-2);
    background: var(--bg-2);
    color: var(--text-2);
    font-size: 12px;
    font-weight: 600;
  }

  .fav :global(svg) {
    color: #ffd24d;
  }

  .fav.on {
    color: var(--text);
    border-color: color-mix(in oklab, var(--c) 60%, transparent);
    background: color-mix(in oklab, var(--c) 15%, transparent);
  }

  .list {
    padding: 4px 0;
  }

  .list input[type='range'] {
    max-width: 260px;
  }

  .list output {
    width: 52px;
    text-align: right;
    font-size: 12px;
    color: var(--muted);
  }

  .led {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--bg-4);
  }

  .led.open {
    background: var(--ok);
    box-shadow: 0 0 8px var(--ok);
  }

  .block {
    padding: 16px 18px 18px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  h3 {
    font-size: 13.5px;
    font-weight: 650;
  }

  h3.sub {
    margin-top: 8px;
  }

  h3 small {
    font-weight: 500;
    color: var(--muted);
    font-size: 12px;
  }

  .rows {
    list-style: none;
    margin: 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .rows li {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 6px 8px 6px 6px;
    border-radius: 12px;
    background: var(--bg-2);
  }

  .rows li.idle {
    opacity: 0.6;
  }

  .av {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 9px;
    display: grid;
    place-items: center;
    font-weight: 750;
    color: #0b0e13;
    background: var(--a);
  }

  .nm {
    width: 150px;
    flex: none;
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .rows input[type='range'],
  .rows select,
  .grow {
    flex: 1;
    min-width: 0;
  }

  .rows output {
    width: 40px;
    text-align: right;
    font-size: 12px;
    color: var(--muted);
  }

  .mini {
    flex: none;
    width: 28px;
    height: 28px;
    border-radius: 8px;
    display: grid;
    place-items: center;
    padding: 0;
    border: 0;
    background: none;
    color: var(--faint);
  }

  .mini:hover {
    color: var(--text);
    background: var(--bg-4);
  }

  .add {
    display: flex;
    gap: 8px;
  }

  .add select {
    flex: 1;
    max-width: 320px;
  }

  .disclose {
    display: flex;
    align-items: center;
    gap: 8px;
    border: 0;
    background: none;
    color: var(--muted);
    font-weight: 600;
    padding: 6px 4px;
  }

  .disclose:hover {
    color: var(--text);
  }

  .chev {
    display: grid;
    transition: transform 0.15s;
  }

  .chev.open {
    transform: rotate(90deg);
  }

  .adv {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }
</style>
