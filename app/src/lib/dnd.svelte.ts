// Pointer-driven drag and drop for application chips.
//
// The browser's own drag-and-drop is avoided on purpose: the desktop shell can swallow it for file
// drops, and a pointer-based drag also gives a smooth custom ghost and works with touch.

const THRESHOLD_PX = 6;

class Dnd {
  /** The application being dragged and where the pointer is. */
  drag = $state<{ name: string; x: number; y: number } | null>(null);
  /** `data-drop` value of the target under the pointer: a channel key or "none". */
  over = $state<string | null>(null);

  begin(e: PointerEvent, name: string, onDrop: (name: string, target: string) => void) {
    if (e.button !== 0 || (e.target as HTMLElement).closest('button, input, [data-nodrag]')) return;
    const startX = e.clientX;
    const startY = e.clientY;
    let active = false;

    const target = (x: number, y: number) =>
      (document.elementFromPoint(x, y)?.closest('[data-drop]') as HTMLElement | null)?.dataset.drop ?? null;

    const move = (ev: PointerEvent) => {
      if (!active) {
        if (Math.hypot(ev.clientX - startX, ev.clientY - startY) < THRESHOLD_PX) return;
        active = true;
        document.body.classList.add('dragging-app');
      }
      this.drag = { name, x: ev.clientX, y: ev.clientY };
      this.over = target(ev.clientX, ev.clientY);
    };

    const finish = (drop: boolean) => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      window.removeEventListener('pointercancel', cancel);
      window.removeEventListener('keydown', key);
      document.body.classList.remove('dragging-app');
      const over = this.over;
      this.drag = null;
      this.over = null;
      if (drop && active && over) onDrop(name, over);
    };
    const up = () => finish(true);
    const cancel = () => finish(false);
    const key = (ev: KeyboardEvent) => ev.key === 'Escape' && finish(false);

    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
    window.addEventListener('pointercancel', cancel);
    window.addEventListener('keydown', key);
  }
}

export const dnd = new Dnd();
