import { api } from "../api";
import type { Settings } from "../types";

export class SettingsStore {
  current = $state<Settings | null>(null);

  async load(): Promise<void> {
    this.current = await api.getSettings();
  }

  /** Applies `patch` immediately, persists it, and rolls back if saving fails. */
  async update(patch: Partial<Settings>): Promise<void> {
    const previous = this.current;
    if (!previous) return;
    const next = { ...previous, ...patch };
    this.current = next;
    try {
      this.current = await api.updateSettings(next);
    } catch (error) {
      if (this.current === next) this.current = previous;
      throw error;
    }
  }
}

export const settings = new SettingsStore();
