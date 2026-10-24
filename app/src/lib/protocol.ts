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
}

export interface EngineInfo {
  api: number;
  recordings_dir: string;
  rec_sources: string[];
  strips: NodeInfo[];
  buses: NodeInfo[];
  hardware: boolean;
}

export interface Stats {
  callbacks: number;
  underruns: number;
  drift_drops: number;
  capture_drops: number;
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

export interface Reply {
  ok: boolean;
  id?: number;
  error?: string;
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  [key: string]: any;
}
