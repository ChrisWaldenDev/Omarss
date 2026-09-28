import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

import { ToastsStore } from "./toasts.svelte";

describe("ToastsStore", () => {
  beforeEach(() => vi.useFakeTimers());
  afterEach(() => vi.useRealTimers());

  it("expires toasts after their timeout", () => {
    const toasts = new ToastsStore();
    toasts.show("broke");
    toasts.info("done", { timeoutMs: 10_000 });
    expect(toasts.items.map((t) => t.kind)).toEqual(["error", "info"]);
    vi.advanceTimersByTime(6000);
    expect(toasts.items.map((t) => t.message)).toEqual(["done"]);
    vi.advanceTimersByTime(4000);
    expect(toasts.items).toEqual([]);
  });

  it("runs an action once and dismisses the toast", async () => {
    const toasts = new ToastsStore();
    const run = vi.fn();
    const id = toasts.info("Marked 3 as read", { action: { label: "Undo", run } });
    await toasts.act(id);
    await toasts.act(id);
    expect(run).toHaveBeenCalledTimes(1);
    expect(toasts.items).toEqual([]);
  });
});
