import { describe, expect, it, vi } from "vitest";

import { listenThenLoad } from "./startup";

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const settle = () => new Promise((resolve) => setTimeout(resolve, 0));

describe("listenThenLoad", () => {
  it("loads only once the listeners are registered", async () => {
    const listening = deferred<() => void>();
    const load = vi.fn(async () => {});
    const unlisten = vi.fn();
    const cleanup = listenThenLoad(() => listening.promise, load);
    await settle();
    expect(load).not.toHaveBeenCalled();

    listening.resolve(unlisten);
    await settle();
    expect(load).toHaveBeenCalledTimes(1);
    cleanup();
    expect(unlisten).toHaveBeenCalledTimes(1);
  });

  it("still loads when the listeners can't be registered", async () => {
    const error = vi.spyOn(console, "error").mockImplementation(() => {});
    const load = vi.fn(async () => {});
    listenThenLoad(() => Promise.reject(new Error("no IPC")), load);
    await settle();
    expect(load).toHaveBeenCalledTimes(1);
    expect(error).toHaveBeenCalled();
    error.mockRestore();
  });

  it("unsubscribes and skips the load when cleaned up before registration finished", async () => {
    const listening = deferred<() => void>();
    const load = vi.fn(async () => {});
    const unlisten = vi.fn();
    const cleanup = listenThenLoad(() => listening.promise, load);
    cleanup();
    listening.resolve(unlisten);
    await settle();
    expect(unlisten).toHaveBeenCalledTimes(1);
    expect(load).not.toHaveBeenCalled();
  });
});
