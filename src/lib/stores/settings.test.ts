import { beforeEach, describe, expect, it, vi } from "vitest";

const api = vi.hoisted(() => ({ getSettings: vi.fn(), updateSettings: vi.fn() }));
vi.mock("../api", () => ({ api }));

import { SettingsStore } from "./settings.svelte";

describe("SettingsStore", () => {
  beforeEach(() => {
    api.getSettings.mockReset().mockResolvedValue({ theme: "system" });
    api.updateSettings.mockReset();
  });

  it("loads settings from the backend", async () => {
    const store = new SettingsStore();
    await store.load();
    expect(store.current).toEqual({ theme: "system" });
  });

  it("applies updates optimistically and keeps the saved value", async () => {
    const store = new SettingsStore();
    await store.load();
    let resolve!: (value: unknown) => void;
    api.updateSettings.mockReturnValue(new Promise((r) => (resolve = r)));

    const pending = store.update({ theme: "dark" });
    expect(store.current).toEqual({ theme: "dark" });
    resolve({ theme: "dark" });
    await pending;
    expect(api.updateSettings).toHaveBeenCalledWith({ theme: "dark" });
    expect(store.current).toEqual({ theme: "dark" });
  });

  it("rolls back when saving fails", async () => {
    const store = new SettingsStore();
    await store.load();
    api.updateSettings.mockRejectedValue(new Error("disk full"));
    await expect(store.update({ theme: "light" })).rejects.toThrow("disk full");
    expect(store.current).toEqual({ theme: "system" });
  });

  it("ignores updates before settings are loaded", async () => {
    const store = new SettingsStore();
    await store.update({ theme: "dark" });
    expect(api.updateSettings).not.toHaveBeenCalled();
  });
});
