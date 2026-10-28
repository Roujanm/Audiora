// Shapes of the engine's control API replies (see docs/CONTROL-API.md).

export interface NodeInfo {
  index: number;
  kind: 'input' | 'tone' | 'bus';
  name: string;
  device_id: string | null;
  sample_rate: number | null;
  channels: number | null;
  period_frames: number | null;
  low_latency: boolean | null;
  exclusive: boolean | null;
  bit_depth: number | null;
  /** Channel key for strips in the channel layout ("game", "chat", "media", "aux", "mic"). */
  channel?: string | null;
  /** Why a configured strip is silent. */
  error?: string | null;
}

export interface EngineInfo {
  api: number;
  recordings_dir: string;
  rec_sources: string[];
  strips: NodeInfo[];
  buses: NodeInfo[];
  hardware: boolean;
  channel_mode?: boolean;
}

export type ChannelKey = 'game' | 'chat' | 'media' | 'aux' | 'music' | 'mic';

export interface ChannelEntry {
  key: ChannelKey;
  name: string;
  strip: number;
  kind: 'apps' | 'mic';
  /** Application channels: `device` = its own virtual output, `direct` = apps captured one by one, `none` = unusable. */
  mode?: 'device' | 'direct' | 'none';
  device_id: string | null;
  device_name: string | null;
  /** The device was picked automatically rather than by the user. */
  auto: boolean;
  online: boolean;
  error: string | null;
}

export interface ChannelsInfo {
  channels: ChannelEntry[];
  master_out: { device_id: string | null; device_name: string | null; auto: boolean };
  mic_out: { device_id: string | null; device_name: string | null };
  /** Unused output where directly captured apps play, unheard. */
  parking?: { device_id: string | null; device_name: string | null; auto: boolean };
  config: {
    master_out: string | null;
    mic_in: string | null;
    mic_out: string | null;
    parking?: string | null;
    mic_raw?: boolean;
    sim_r?: boolean;
    devices: Record<string, string | null>;
  };
}

export interface ChannelPresets {
  factory: string[];
  user: string[];
  favorites: string[];
}

export interface Stats {
  callbacks: number;
  underruns: number;
  drift_drops: number;
  capture_drops: number;
  device_errors: number;
}

export interface RecStatus {
  recording: boolean;
  path?: string;
  sources?: string[];
  seconds?: number;
  dropped_frames?: number;
  error?: string | null;
  dir?: string;
}

export interface TakeEntry {
  name: string;
  path: string;
  bytes: number;
  modified_ms: number;
  channels: number;
  sample_rate: number;
  seconds: number;
}

export interface DeviceEntry {
  id: string;
  name: string;
  flow: 'render' | 'capture';
  default: boolean;
  sample_rate: number;
  channels: number;
}

export type HwKind =
  | { type: 'range'; min: number; max: number; step: number; unit: string }
  | { type: 'toggle' }
  | { type: 'choice'; options: string[] };

export interface HwControl {
  path: string;
  label: string;
  group: string;
  kind: HwKind;
  value: number | null;
}

export interface HwInfo {
  model: string;
  serial: string;
  firmware: string;
  simulated: boolean;
  inputs: number;
  outputs: number;
}

export interface LicenseStatus {
  state: 'dev-build' | 'none' | 'valid' | 'invalid';
  edition?: string;
  issued_to?: string;
  error?: string;
}

export interface AppSessionEntry {
  instance_id: string;
  device_id: string;
  device_name: string;
  pid: number;
  name: string;
  volume: number;
  muted: boolean;
  peak: number;
  active: boolean;
  system_sounds: boolean;
  /** Channel this application is assigned to (survives restarts of the app). */
  route?: ChannelKey | null;
  /** Channel whose device the application is playing into right now. */
  playing_in?: ChannelKey | null;
}

export interface Reply {
  ok: boolean;
  id?: number;
  error?: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [key: string]: any;
}
