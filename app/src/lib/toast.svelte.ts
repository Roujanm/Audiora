// Tiny notification queue: `toast('Saved')` shows a message that dismisses itself.

export type ToastKind = 'info' | 'ok' | 'error';

export interface ToastItem {
  id: number;
  text: string;
  kind: ToastKind;
}

class ToastQueue {
  items = $state<ToastItem[]>([]);
  #next = 1;

  push(text: string, kind: ToastKind = 'info', ms = 3200) {
    const id = this.#next++;
    this.items = [...this.items.slice(-3), { id, text, kind }];
    setTimeout(() => this.dismiss(id), kind === 'error' ? Math.max(ms, 6000) : ms);
  }

  dismiss(id: number) {
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toasts = new ToastQueue();
export const toast = (text: string, kind: ToastKind = 'info') => toasts.push(text, kind);
