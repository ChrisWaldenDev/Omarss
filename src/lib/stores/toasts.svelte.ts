export interface Toast {
  id: number;
  message: string;
}

/** Short-lived messages for things that fail in the background. */
export class ToastsStore {
  items = $state<Toast[]>([]);
  #next = 0;

  show(message: string, timeoutMs = 6000): void {
    const id = ++this.#next;
    this.items = [...this.items, { id, message }];
    setTimeout(() => this.dismiss(id), timeoutMs);
  }

  dismiss(id: number): void {
    this.items = this.items.filter((t) => t.id !== id);
  }
}

export const toasts = new ToastsStore();
