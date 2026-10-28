// Desktop-only features: live updates and starting with Windows. In a browser every call is a
// no-op, so the rest of the UI never needs to know where it runs.

import { engine } from './engine.svelte.ts';
import { toast } from './toast.svelte.ts';

const CHECK_EVERY_MS = 6 * 60 * 60 * 1000;

function inTauri(): boolean {
  return typeof window !== 'undefined' && '__TAURI_INTERNALS__' in window;
}

type Pending = Awaited<ReturnType<typeof import('@tauri-apps/plugin-updater').check>>;

class Desktop {
  readonly available = inTauri();
  version = $state('');
  /** An update that can be installed now. */
  update = $state<{ version: string; notes: string } | null>(null);
  phase = $state<'idle' | 'checking' | 'downloading' | 'installing'>('idle');
  /** 0..1 while downloading. */
  progress = $state(0);
  dismissed = $state(false);
  autostart = $state(false);

  #pending: Pending = null;
  #started = false;

  async start() {
    if (!this.available || this.#started) return;
    this.#started = true;
    const { getVersion } = await import('@tauri-apps/api/app');
    this.version = await getVersion();
    await this.#initAutostart();
    // look for an update shortly after start, so a new version greets the user when Audiora opens
    setTimeout(() => void this.check(false), 3000);
    setInterval(() => void this.check(false), CHECK_EVERY_MS);
  }

  /** Starting with Windows is on by default; after the first run it follows the user's choice. */
  async #initAutostart() {
    try {
      const { enable, isEnabled } = await import('@tauri-apps/plugin-autostart');
      if (!localStorage.getItem('audiora.autostart-init')) {
        localStorage.setItem('audiora.autostart-init', '1');
        await enable();
      }
      this.autostart = await isEnabled();
    } catch {
      /* not available: the switch stays off */
    }
  }

  async setAutostart(on: boolean) {
    if (!this.available) return;
    try {
      const { enable, disable, isEnabled } = await import('@tauri-apps/plugin-autostart');
      await (on ? enable() : disable());
      this.autostart = await isEnabled();
      toast(on ? 'Audiora will start with Windows' : 'Audiora will no longer start with Windows', 'ok');
    } catch (e) {
      toast(String(e), 'error');
    }
  }

  async check(manual: boolean) {
    if (!this.available || this.phase !== 'idle') return;
    this.phase = 'checking';
    try {
      const { check } = await import('@tauri-apps/plugin-updater');
      const u = await check();
      if (u) {
        this.#pending = u;
        this.update = { version: u.version, notes: u.body ?? '' };
        this.dismissed = false;
      } else if (manual) {
        toast(`Audiora ${this.version} is up to date`, 'ok');
      }
    } catch (e) {
      if (manual) toast(`Could not check for updates: ${e}`, 'error');
    } finally {
      this.phase = 'idle';
    }
  }

  /** Downloads and installs the pending update, then restarts Audiora. */
  async install() {
    const u = this.#pending;
    if (!u || this.phase !== 'idle') return;
    this.phase = 'downloading';
    this.progress = 0;
    try {
      let total = 0;
      let got = 0;
      // download while the sound keeps playing; only the install itself needs the engine stopped
      await u.download((ev) => {
        if (ev.event === 'Started') total = ev.data.contentLength ?? 0;
        else if (ev.event === 'Progress') {
          got += ev.data.chunkLength;
          this.progress = total ? got / total : 0;
        }
      });
      // the engine's files are replaced too: stop it (and keep the UI from restarting it) first
      this.phase = 'installing';
      const { invoke } = await import('@tauri-apps/api/core');
      engine.suspend();
      await invoke('stop_engine_now');
      await u.install();
      const { relaunch } = await import('@tauri-apps/plugin-process');
      await relaunch();
    } catch (e) {
      this.phase = 'idle';
      engine.resume();
      toast(`Update failed: ${e}`, 'error');
    }
  }
}

export const desktop = new Desktop();
