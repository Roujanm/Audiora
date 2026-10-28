// Reactive client for the Audiora engine's WebSocket control API.
// Components read `engine.*` and call its methods; nothing else touches the socket.

import { stepMeter, stepSpectrum, type MeterState } from './ballistics.ts';
import { shortName } from './theme.ts';
import { toast } from './toast.svelte.ts';
import type {
  AppSessionEntry,
  ChannelKey,
  ChannelPresets,
  ChannelsInfo,
  DeviceEntry,
  EngineInfo,
  HwControl,
  HwInfo,
  LicenseStatus,
  RecStatus,
  Reply,
  Stats,
  TakeEntry,
} from './protocol.ts';

// ?ws=PORT points a development build at a separate test engine instead of the user's own
const WS_URL = `ws://127.0.0.1:${(typeof location !== 'undefined' && new URLSearchParams(location.search).get('ws')) || '7879'}`;
const STATE_POLL_MS = 1000;
const HW_POLL_MS = 100;
const DIRTY_GRACE_MS = 600;

type Pending = { resolve: (r: Reply) => void; reject: (e: Error) => void; timer: number };

function loadAliases(): Record<string, string> {
  try {
    const raw = JSON.parse(localStorage.getItem('audiora.names') ?? '{}');
    return raw && typeof raw === 'object' ? (raw as Record<string, string>) : {};
  } catch {
    return {};
  }
}

function loadActivePresets(): Record<string, string> {
  try {
    const raw = JSON.parse(localStorage.getItem('audiora.activePreset') ?? '{}');
    return raw && typeof raw === 'object' ? (raw as Record<string, string>) : {};
  } catch {
    return {};
  }
}

/** Applications that are not worth routing: Audiora's own processes. */
const IGNORED_APPS = new Set(['audiora', 'audiora-app', 'audiora-svc', 'msedgewebview2', 'audiodg']);

export interface AppGroup {
  /** Executable name as Windows reports it ("chrome"). */
  name: string;
  sessions: AppSessionEntry[];
  /** Strongest live level among its sessions, 0..1. */
  peak: number;
  active: boolean;
  /** Channel the user assigned it to. */
  route: ChannelKey | null;
  /** Channel whose device it is actually playing into. */
  playingIn: ChannelKey | null;
  volume: number;
  muted: boolean;
}

function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

class EngineClient {
  connected = $state(false);
  connecting = $state(true);
  lastError = $state('');

  info = $state<EngineInfo | null>(null);
  params = $state<Record<string, number>>({});
  /** Peak meters with ballistics (instant attack, steady decay, peak hold). */
  meters = $state<Record<string, MeterState>>({});
  /** Other meters as reported: gain reduction in dB, gate open flag. */
  gauges = $state<Record<string, number>>({});
  stats = $state<Stats>({ callbacks: 0, underruns: 0, drift_drops: 0, capture_drops: 0, device_errors: 0 });
  /** The engine's recent glitches, newest first: unix time, kind and where it happened. */
  glitchLog = $state<{ t: number; kind: string; place: string }[]>([]);
  rec = $state<RecStatus>({ recording: false });
  takes = $state<TakeEntry[]>([]);
  presets = $state<string[]>([]);
  devices = $state<DeviceEntry[]>([]);
  license = $state<LicenseStatus | null>(null);
  apps = $state<AppSessionEntry[]>([]);

  /** Channel layout (Game, Chat, Media, Aux, Mic) with the device behind each. */
  channels = $state<ChannelsInfo | null>(null);
  /** Application name -> channel key. */
  routes = $state<Record<string, ChannelKey>>({});
  /** Presets per channel: built-in, user-made and favourites. */
  chPresets = $state<Record<string, ChannelPresets>>({});
  /** Application name -> channel -> preset applied automatically when the app starts. */
  profiles = $state<Record<string, Record<string, string>>>({});
  /** Last preset applied to each channel (remembered in this window only). */
  activePreset = $state<Record<string, string>>(loadActivePresets());

  /** Programme loudness of the master bus in LUFS (momentary, short-term, integrated). */
  loudness = $state({ m: -120, s: -120, i: -120 });
  /** Smoothed spectrum in dBFS while an analyser source is selected, else null. */
  spectrum = $state<number[] | null>(null);
  bandHz = $state<number[]>([]);
  /** User-chosen channel names, kept per device in this browser / window. */
  aliases = $state<Record<string, string>>(loadAliases());

  hwInfo = $state<HwInfo | null>(null);
  hwControls = $state<HwControl[]>([]);
  hwMeters = $state<Record<string, number>>({});

  #ws: WebSocket | null = null;
  #nextId = 1;
  #pending = new Map<number, Pending>();
  #meterPaths: string[] = [];
  #lastMeterAt = 0;
  #dirty = new Map<string, number>();
  #timers: number[] = [];
  #started = false;
  #lastAnalysisAt = 0;

  /** True when the engine runs the Game/Chat/Media/Aux/Mic layout. */
  get channelMode(): boolean {
    return !!this.info?.channel_mode;
  }

  /** Detected applications, one entry per program however many audio sessions it has. */
  appGroups = $derived.by<AppGroup[]>(() => {
    const byName = new Map<string, AppGroup>();
    for (const a of this.apps) {
      if (a.system_sounds || a.pid === 0) continue;
      const key = a.name.toLowerCase();
      if (IGNORED_APPS.has(key)) continue;
      let g = byName.get(key);
      if (!g) {
        g = {
          name: a.name,
          sessions: [],
          peak: 0,
          active: false,
          route: this.routes[key] ?? null,
          playingIn: null,
          volume: a.volume,
          muted: a.muted,
        };
        byName.set(key, g);
      }
      g.sessions.push(a);
      g.peak = Math.max(g.peak, a.peak);
      g.active ||= a.active;
      g.playingIn ||= a.playing_in ?? null;
      // the session on the channel device carries the app's volume
      if (a.playing_in) {
        g.volume = a.volume;
        g.muted = a.muted;
      }
    }
    // remembered routes of applications that are not running right now
    for (const [key, route] of Object.entries(this.routes)) {
      if (!byName.has(key)) {
        byName.set(key, { name: key, sessions: [], peak: 0, active: false, route, playingIn: null, volume: 1, muted: false });
      }
    }
    return [...byName.values()].sort((a, b) => Number(b.active) - Number(a.active) || a.name.localeCompare(b.name));
  });

  start() {
    if (this.#started) return;
    this.#started = true;
    void this.#connect();
  }

  #suspended = false;

  /** Disconnects and stops reconnecting (an update is about to replace the engine). */
  suspend() {
    this.#suspended = true;
    this.#ws?.close();
  }

  resume() {
    if (!this.#suspended) return;
    this.#suspended = false;
    void this.#connect();
  }

  async #connect() {
    this.connecting = true;
    if (inTauri()) {
      try {
        const { invoke } = await import('@tauri-apps/api/core');
        await invoke('ensure_engine');
      } catch (e) {
        this.lastError = `Could not start the engine: ${e}`;
      }
    }
    const ws = new WebSocket(WS_URL);
    this.#ws = ws;
    ws.onopen = () => void this.#bootstrap();
    ws.onmessage = (ev) => this.#onMessage(ev.data as string);
    ws.onclose = () => {
      this.#teardown();
      if (!this.#suspended) setTimeout(() => void this.#connect(), 1000);
    };
    ws.onerror = () => {
      /* onclose follows and handles the retry */
    };
  }

  #teardown() {
    this.connected = false;
    this.connecting = true;
    this.#timers.forEach(clearInterval);
    this.#timers = [];
    for (const p of this.#pending.values()) {
      clearTimeout(p.timer);
      p.reject(new Error('disconnected'));
    }
    this.#pending.clear();
  }

  async #bootstrap() {
    try {
      this.info = (await this.call('info')) as unknown as EngineInfo;
      await this.refreshState();
      const sub = await this.call('subscribe', { hz: 30 });
      this.#meterPaths = sub.paths as string[];
      this.bandHz = (sub.band_hz as number[]) ?? [];
      this.#lastMeterAt = performance.now();
      await Promise.all([
        this.refreshPresets(),
        this.refreshDevices(),
        this.refreshRec(),
        this.refreshLicense(),
        this.refreshTakes().catch(() => {}),
      ]);
      if (this.info?.hardware) await this.refreshHardware();
      if (this.channelMode) {
        await Promise.all([this.refreshChannels(), this.refreshRoutes(), this.refreshProfiles(), this.refreshApps().catch(() => {})]);
        for (const c of ['game', 'chat', 'media', 'aux', 'mic']) await this.refreshChannelPresets(c);
        this.#timers.push(window.setInterval(() => void this.refreshApps().catch(() => {}), 1500));
      }
      this.connected = true;
      this.connecting = false;
      this.lastError = '';

      this.#timers.push(window.setInterval(() => void this.refreshState().catch(() => {}), STATE_POLL_MS));
      this.#timers.push(
        window.setInterval(() => {
          if (this.rec.recording) void this.refreshRec().catch(() => {});
        }, 500),
      );
      if (this.info?.hardware) {
        this.#timers.push(window.setInterval(() => void this.#pollHwMeters().catch(() => {}), HW_POLL_MS));
      }
    } catch (e) {
      this.lastError = String(e);
    }
  }

  #onMessage(text: string) {
    let msg: Reply & { event?: string; v?: number[] };
    try {
      msg = JSON.parse(text);
    } catch {
      return;
    }
    if (msg.event === 'meters' && msg.v) {
      this.#applyMeters(msg.v);
      return;
    }
    if (msg.event === 'analysis') {
      this.#applyAnalysis(msg as unknown as { m: number; s: number; i: number; spectrum: number[] | null });
      return;
    }
    if (msg.id != null) {
      const p = this.#pending.get(msg.id);
      if (!p) return;
      this.#pending.delete(msg.id);
      clearTimeout(p.timer);
      if (msg.ok) p.resolve(msg);
      else p.reject(new Error(msg.error ?? 'request failed'));
    }
  }

  #applyMeters(values: number[]) {
    const now = performance.now();
    const elapsed = now - this.#lastMeterAt;
    this.#lastMeterAt = now;
    const meters: Record<string, MeterState> = {};
    const gauges: Record<string, number> = {};
    this.#meterPaths.forEach((path, i) => {
      const v = values[i] ?? 0;
      if (path.endsWith('/peak_l') || path.endsWith('/peak_r') || path.endsWith('/in_peak')) {
        meters[path] = stepMeter(this.meters[path], v, now, elapsed);
      } else {
        gauges[path] = v;
      }
    });
    this.meters = meters;
    this.gauges = gauges;
  }

  #applyAnalysis(a: { m: number; s: number; i: number; spectrum: number[] | null }) {
    const now = performance.now();
    const elapsed = now - this.#lastAnalysisAt;
    this.#lastAnalysisAt = now;
    this.loudness = { m: a.m, s: a.s, i: a.i };
    this.spectrum = a.spectrum ? stepSpectrum(this.spectrum, a.spectrum, elapsed) : null;
  }

  /** Which signal feeds the spectrum analyser (`strip/0`, `bus/0`, ...) or null to switch it off. */
  setSpectrumSource(source: string | null) {
    this.call('analysis_source', { source }).catch(() => {});
    if (source === null) this.spectrum = null;
  }

  resetLoudness() {
    this.call('loudness_reset')
      .then(() => toast('Loudness measurement restarted'))
      .catch((e) => (this.lastError = String(e)));
  }

  // ---- channel names -------------------------------------------------------

  #aliasKey(kind: 'strip' | 'bus', index: number): string | null {
    const n = kind === 'strip' ? this.info?.strips[index] : this.info?.buses[index];
    if (!n) return null;
    return n.kind === 'tone' ? 'tone' : (n.device_id ?? `${kind}:${index}`);
  }

  /** Name to show for a strip or bus: the user's own, else a tidied device name. */
  labelOf(kind: 'strip' | 'bus', index: number): string {
    const n = kind === 'strip' ? this.info?.strips[index] : this.info?.buses[index];
    if (!n) return '—';
    const key = this.#aliasKey(kind, index);
    if (kind === 'strip' && 'channel' in n && n.channel) return n.name;
    return (key && this.aliases[key]) || (n.kind === 'tone' ? 'Tone generator' : shortName(n.name));
  }

  rename(kind: 'strip' | 'bus', index: number, name: string) {
    const key = this.#aliasKey(kind, index);
    if (!key) return;
    const next = { ...this.aliases };
    const clean = name.trim().slice(0, 40);
    if (clean) next[key] = clean;
    else delete next[key];
    this.aliases = next;
    try {
      localStorage.setItem('audiora.names', JSON.stringify(next));
    } catch {
      /* private mode: the name just is not remembered */
    }
  }

  /** Sends a command and resolves with the reply; rejects on `ok: false`. */
  call(cmd: string, fields: Record<string, unknown> = {}): Promise<Reply> {
    return new Promise((resolve, reject) => {
      const ws = this.#ws;
      if (!ws || ws.readyState !== WebSocket.OPEN) {
        reject(new Error('not connected'));
        return;
      }
      const id = this.#nextId++;
      const timer = window.setTimeout(() => {
        this.#pending.delete(id);
        reject(new Error(`${cmd} timed out`));
      }, 5000);
      this.#pending.set(id, { resolve, reject, timer });
      ws.send(JSON.stringify({ id, cmd, ...fields }));
    });
  }

  // ---- mixer parameters -------------------------------------------------

  get(path: string, fallback = 0): number {
    return this.params[path] ?? fallback;
  }

  /** Optimistic update; the engine clamps and the next poll reconciles. */
  set(path: string, value: number | boolean) {
    const v = typeof value === 'boolean' ? (value ? 1 : 0) : value;
    this.params[path] = v;
    this.#dirty.set(path, performance.now());
    // a hand-made change means the channel no longer matches the preset it was loaded from
    const sound = /^strip\/(\d+)\/(eq\/|boost_db|smart\/|denoise|gate\/|comp\/|lowcut\/|deess\/|limiter\/)/.exec(path);
    const channel = sound ? this.channelOf(+sound[1]) : null;
    if (channel && this.activePreset[channel]) this.#remember(channel, null);
    this.call('set', { path, value: v }).catch((e) => (this.lastError = String(e)));
  }

  toggle(path: string) {
    this.set(path, this.get(path) < 0.5);
  }

  async refreshState() {
    const r = await this.call('state');
    const now = performance.now();
    const incoming = r.params as Record<string, number>;
    const next: Record<string, number> = { ...this.params };
    for (const [k, v] of Object.entries(incoming)) {
      const touched = this.#dirty.get(k);
      if (touched !== undefined && now - touched < DIRTY_GRACE_MS) continue;
      next[k] = v;
    }
    this.params = next;
    this.stats = r.stats as Stats;
    this.glitchLog = (r.glitches as { t: number; kind: string; place: string }[] | undefined) ?? [];
  }

  // ---- presets ----------------------------------------------------------

  async refreshPresets() {
    this.presets = (await this.call('preset_list')).presets as string[];
  }

  async savePreset(name: string) {
    await this.call('preset_save', { name });
    await this.refreshPresets();
  }

  async loadPreset(name: string) {
    await this.call('preset_load', { name });
    await this.refreshState();
  }

  async deletePreset(name: string) {
    await this.call('preset_delete', { name });
    await this.refreshPresets();
  }

  // ---- recording --------------------------------------------------------

  async refreshRec() {
    this.rec = (await this.call('rec_status')) as unknown as RecStatus;
  }

  async refreshTakes() {
    this.takes = (await this.call('rec_list')).takes as TakeEntry[];
  }

  async recStart(sources: string[]) {
    await this.call('rec_start', { sources });
    await this.refreshRec();
  }

  async recStop() {
    const r = await this.call('rec_stop');
    await this.refreshRec();
    await this.refreshTakes().catch(() => {});
    return r;
  }

  /** Shows a take in Explorer (desktop app only). */
  async revealTake(path: string): Promise<boolean> {
    if (!inTauri()) return false;
    const { invoke } = await import('@tauri-apps/api/core');
    await invoke('reveal_in_folder', { path });
    return true;
  }

  get canReveal(): boolean {
    return inTauri();
  }

  // ---- application volumes (Windows audio sessions) ------------------------

  async refreshApps() {
    const now = performance.now();
    const incoming = (await this.call('apps')).apps as AppSessionEntry[];
    // keep a slider the user is dragging from jumping back to a stale value
    this.apps = incoming.map((a) => {
      const touched = this.#dirty.get(`app:${a.instance_id}`);
      if (touched === undefined || now - touched > DIRTY_GRACE_MS) return a;
      const old = this.apps.find((o) => o.instance_id === a.instance_id && o.device_id === a.device_id);
      return old ? { ...a, volume: old.volume, muted: old.muted } : a;
    });
  }

  setApp(app: AppSessionEntry, change: { volume?: number; mute?: boolean }) {
    this.#dirty.set(`app:${app.instance_id}`, performance.now());
    this.apps = this.apps.map((a) =>
      a.instance_id === app.instance_id && a.device_id === app.device_id
        ? { ...a, volume: change.volume ?? a.volume, muted: change.mute ?? a.muted }
        : a,
    );
    this.call('app_set', { device: app.device_id, instance_id: app.instance_id, ...change }).catch(
      (e) => (this.lastError = String(e)),
    );
  }

  // ---- channels, routing, channel presets, game profiles -------------------

  async refreshChannels() {
    this.channels = (await this.call('channels')) as unknown as ChannelsInfo;
  }

  async refreshRoutes() {
    this.routes = (await this.call('routes')).routes as Record<string, ChannelKey>;
  }

  async refreshProfiles() {
    this.profiles = (await this.call('app_profiles')).profiles as Record<string, Record<string, string>>;
  }

  /** Sends an application to a channel (or `null` to give it back to Windows' own routing). */
  async routeApp(name: string, channel: ChannelKey | null) {
    const key = name.toLowerCase();
    const before = this.routes;
    // move the chip at once; the reply and the next poll confirm it
    const next = { ...this.routes };
    if (channel) next[key] = channel;
    else delete next[key];
    this.routes = next;
    try {
      const r = await this.call('app_route', { app: name, channel });
      await Promise.all([this.refreshRoutes(), this.refreshApps()]);
      return { live: !!r.live };
    } catch (e) {
      this.routes = before;
      throw e;
    }
  }

  async refreshChannelPresets(channel: string) {
    const r = await this.call('cpreset_list', { channel });
    this.chPresets = {
      ...this.chPresets,
      [channel]: { factory: r.factory as string[], user: r.user as string[], favorites: r.favorites as string[] },
    };
  }

  #remember(channel: string, name: string | null) {
    const next = { ...this.activePreset };
    if (name) next[channel] = name;
    else delete next[channel];
    this.activePreset = next;
    try {
      localStorage.setItem('audiora.activePreset', JSON.stringify(next));
    } catch {
      /* the choice is just not remembered */
    }
  }

  async loadChannelPreset(channel: string, name: string) {
    await this.call('cpreset_load', { channel, name });
    this.#remember(channel, name);
    await this.refreshState();
  }

  async saveChannelPreset(channel: string, name: string) {
    await this.call('cpreset_save', { channel, name });
    this.#remember(channel, name);
    await this.refreshChannelPresets(channel);
  }

  async deleteChannelPreset(channel: string, name: string) {
    await this.call('cpreset_delete', { channel, name });
    if (this.activePreset[channel] === name) this.#remember(channel, null);
    await this.refreshChannelPresets(channel);
  }

  async favoriteChannelPreset(channel: string, name: string, on: boolean) {
    await this.call('cpreset_favorite', { channel, name, on });
    await this.refreshChannelPresets(channel);
  }

  /** Makes `preset` load by itself whenever `app` starts (null removes the rule). */
  async setProfile(app: string, channel: string, preset: string | null) {
    await this.call('app_profile_set', { app, channel, preset });
    await this.refreshProfiles();
  }

  /** Saves device choices; they apply when the engine restarts. */
  async configSet(fields: Record<string, unknown>) {
    await this.call('config_set', fields);
    await this.refreshChannels();
  }

  /** Stops the engine; the desktop app starts a fresh one a moment later. */
  async restartEngine() {
    await this.call('shutdown').catch(() => {});
  }

  /** Channel key -> display name, colour index = strip index. */
  channelOf(strip: number): ChannelKey | null {
    return (this.info?.strips[strip]?.channel as ChannelKey | undefined) ?? null;
  }

  // ---- devices, license, hardware ---------------------------------------

  async refreshDevices() {
    this.devices = (await this.call('devices')).devices as DeviceEntry[];
  }

  async refreshLicense() {
    this.license = (await this.call('license_status')) as unknown as LicenseStatus;
  }

  async refreshHardware() {
    this.hwInfo = (await this.call('hw_info')) as unknown as HwInfo;
    this.hwControls = (await this.call('hw_controls')).controls as HwControl[];
  }

  async #pollHwMeters() {
    this.hwMeters = (await this.call('hw_meters')).meters as Record<string, number>;
  }

  async setHw(path: string, value: number | boolean) {
    const v = typeof value === 'boolean' ? (value ? 1 : 0) : value;
    this.hwControls = this.hwControls.map((c) => (c.path === path ? { ...c, value: v } : c));
    try {
      const r = await this.call('hw_set', { path, value: v });
      this.hwControls = this.hwControls.map((c) => (c.path === path ? { ...c, value: r.value as number } : c));
      // a change can have side effects (source change drops +48 V): re-read everything
      this.hwControls = (await this.call('hw_controls')).controls as HwControl[];
    } catch (e) {
      this.lastError = String(e);
      await this.refreshHardware();
    }
  }

  // ---- helpers for components -------------------------------------------

  peakDb(path: string): number {
    return this.meters[path]?.db ?? -120;
  }

  holdDb(path: string): number {
    return this.meters[path]?.hold ?? -120;
  }

  gauge(path: string): number {
    return this.gauges[path] ?? 0;
  }
}

export const engine = new EngineClient();
