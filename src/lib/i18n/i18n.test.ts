import { describe, expect, it } from "vitest";

import en from "./en.json";
import { t } from "./index";

describe("t", () => {
  it("returns the English string", () => {
    expect(t("view.all")).toBe("All articles");
  });

  it("fills placeholders and formats numbers for the locale", () => {
    expect(t("sidebar.unreadCount", { count: 12345 }, "en")).toBe("12,345 unread");
    expect(t("reader.byAuthor", { author: "Ana" })).toBe("by Ana");
  });

  it("leaves unknown placeholders visible", () => {
    expect(t("reader.byAuthor", {})).toBe("by {author}");
  });

  it("has no empty strings", () => {
    for (const [key, value] of Object.entries(en)) expect(value, key).not.toBe("");
  });
});
