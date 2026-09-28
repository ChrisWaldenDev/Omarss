export interface ToastAction {
  label: string;
  run: () => void | Promise<void>;
}

export interface Toast {
  id: number;
  message: string;
  kind: "error" | "info";
  action?: ToastAction;
}

/** Short-lived messages: background failures, and confirmations that offer an action. */
export class ToastsStore {
  items = $state<Toast[]>([]);
  #next = 0;

  /** An error message. */
  show(message: string, timeoutMs = 6000): number {
    return this.#add({ message, kind: "error" }, timeoutMs);
  }

  /** A confirmation, optionally with an action such as "Undo". */
  info(message: string, options: { action?: ToastAction; timeoutMs?: number } = {}): number {
    return this.#add({ message, kind: "info", action: options.action }, options.timeoutMs ?? 6000);
  }

  dismiss(id: number): void {
    this.items = this.items.filter((t) => t.id !== id);
  }

  /** Runs a toast's action once and dismisses it. */
  async act(id: number): Promise<void> {
    const toast = this.items.find((t) => t.id === id);
    this.dismiss(id);
    await toast?.action?.run();
  }

  #add(toast: Omit<Toast, "id">, timeoutMs: number): number {
    const id = ++this.#next;
    this.items = [...this.items, { id, ...toast }];
    setTimeout(() => this.dismiss(id), timeoutMs);
    return id;
  }
}

export const toasts = new ToastsStore();
