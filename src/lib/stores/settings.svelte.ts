import { api, errorMessage } from "../api";
import { t } from "../i18n";
import type { Settings } from "../types";
import { toasts } from "./toasts.svelte";

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

  /** `update` for settings controls: a failure is reported as a toast. */
  async save(patch: Partial<Settings>): Promise<boolean> {
    try {
      await this.update(patch);
      return true;
    } catch (error) {
      toasts.show(t("settings.saveError", { message: errorMessage(error) }));
      return false;
    }
  }
}

export const settings = new SettingsStore();
