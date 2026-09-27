import { beforeEach, describe, expect, it, vi } from "vitest";

const invoke = vi.hoisted(() => vi.fn());
vi.mock("@tauri-apps/api/core", () => ({ invoke }));

import { api, ApiError } from "./api";

describe("api", () => {
  beforeEach(() => {
    invoke.mockReset();
  });

  it("calls the generated command and returns its data", async () => {
    invoke.mockResolvedValue({ theme: "dark" });
    await expect(api.updateSettings({ theme: "dark" })).resolves.toEqual({ theme: "dark" });
    expect(invoke).toHaveBeenCalledWith("update_settings", { settings: { theme: "dark" } });
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
