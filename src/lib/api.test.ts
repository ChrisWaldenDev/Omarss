import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { api, ApiError } from "./api";
import { defaultSettings } from "./testing/settings";

describe("api", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("calls the generated command and returns its data", async () => {
    const settings = defaultSettings({ theme: "dark" });
    invoke.mockResolvedValue(settings);
    await expect(api.updateSettings(settings)).resolves.toEqual(settings);
    expect(invoke).toHaveBeenCalledWith("update_settings", { settings });
  });

  it("throws backend errors as ApiError with their kind", async () => {
    invoke.mockRejectedValue({ kind: "not_found", message: "Article 7 not found" });
    const error = await api.getArticle(7).catch((e: unknown) => e);
    expect(error).toBeInstanceOf(ApiError);
    expect((error as ApiError).kind).toBe("not_found");
    expect((error as ApiError).message).toBe("Article 7 not found");
  });

  it("wraps every generated command", async () => {
    const { commands } = await import("./types");
    expect(Object.keys(api).sort()).toEqual(Object.keys(commands).sort());
  });
});
