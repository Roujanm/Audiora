// Whether anybody is looking at Audiora. A mixer sits open all day, often behind a game: when it is
// minimised nothing on screen needs to move, and when it is in the background the meters can tick
// slower. The engine keeps running at full rate either way; only drawing slows down.

class Activity {
  /** The window is shown (not minimised, not a hidden page). */
  visible = $state(true);
  /** The window has the keyboard focus. */
  focused = $state(true);

  /** Animations run and meters tick at full rate. */
  get lively() {
    return this.visible && this.focused;
  }

  start() {
    const sync = () => {
      this.visible = document.visibilityState === 'visible' && this.visible;
    };
    document.addEventListener('visibilitychange', () => {
      this.visible = document.visibilityState === 'visible';
      if (this.visible) void this.#checkMinimized();
    });
    window.addEventListener('focus', () => {
      this.focused = true;
      void this.#checkMinimized();
    });
    window.addEventListener('blur', () => (this.focused = false));
    this.focused = document.hasFocus();
    sync();
    void this.#watchWindow();
  }

  // WebView2 keeps reporting "visible" while the window is minimised, so ask Tauri about it
  async #watchWindow() {
    if (!('__TAURI_INTERNALS__' in window)) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      const w = getCurrentWindow();
      await w.onResized(() => void this.#checkMinimized());
      await this.#checkMinimized();
    } catch {
      /* not in the desktop app */
    }
  }

  async #checkMinimized() {
    if (!('__TAURI_INTERNALS__' in window)) return;
    try {
      const { getCurrentWindow } = await import('@tauri-apps/api/window');
      this.visible = !(await getCurrentWindow().isMinimized()) && document.visibilityState === 'visible';
    } catch {
      /* ignore */
    }
  }
}

export const activity = new Activity();
