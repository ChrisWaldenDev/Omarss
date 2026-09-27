import { describe, expect, it } from "vitest";

import { fileSize, letterAvatar, relativeTime } from "./format";

const NOW = Date.UTC(2026, 8, 27, 12, 0, 0);
const nowSec = NOW / 1000;

describe("relativeTime", () => {
  it("uses relative units for the last week", () => {
    expect(relativeTime(nowSec - 10, NOW, "en")).toBe("now");
    expect(relativeTime(nowSec - 5 * 60, NOW, "en")).toBe("5 min. ago");
    expect(relativeTime(nowSec - 3 * 3600, NOW, "en")).toBe("3 hr. ago");
    expect(relativeTime(nowSec - 26 * 3600, NOW, "en")).toBe("yesterday");
    expect(relativeTime(nowSec - 3 * 86400, NOW, "en")).toBe("3 days ago");
  });

  it("falls back to a date for older items", () => {
    expect(relativeTime(nowSec - 20 * 86400, NOW, "en")).toBe("Sep 7");
    expect(relativeTime(Date.UTC(2025, 0, 15, 12) / 1000, NOW, "en")).toBe("Jan 15, 2025");
  });

  it("treats future timestamps as now", () => {
    expect(relativeTime(nowSec + 120, NOW, "en")).toBe("now");
  });
});

describe("fileSize", () => {
  it("picks a readable unit", () => {
    expect(fileSize(512, "en")).toBe("512 byte");
    expect(fileSize(2_500, "en")).toBe("2.5 kB");
    expect(fileSize(24_000_000, "en")).toBe("24 MB");
  });
});

describe("letterAvatar", () => {
  it("is deterministic and uses the first letter", () => {
    expect(letterAvatar("systems weekly")).toEqual(letterAvatar("systems weekly"));
    expect(letterAvatar("  systems weekly").letter).toBe("S");
    expect(letterAvatar("").letter).toBe("?");
    expect(letterAvatar("Élan").letter).toBe("É");
  });
});
