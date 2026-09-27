import { describe, expect, it } from "vitest";

import { scrollToReveal, visibleRange } from "./virtual";

describe("visibleRange", () => {
  it("renders only the rows in view plus overscan", () => {
    expect(visibleRange(0, 500, 100, 100_000, 2)).toEqual({ start: 0, end: 8, offset: 0 });
    expect(visibleRange(5_000_000, 500, 100, 100_000, 2)).toEqual({
      start: 49_998,
      end: 50_008,
      offset: 4_999_800,
    });
  });

  it("clamps at the ends", () => {
    expect(visibleRange(0, 500, 100, 3)).toEqual({ start: 0, end: 3, offset: 0 });
    expect(visibleRange(10_000, 500, 100, 20, 2).end).toBe(20);
    expect(visibleRange(0, 500, 100, 0)).toEqual({ start: 0, end: 0, offset: 0 });
  });
});

describe("scrollToReveal", () => {
  it("scrolls only when the row is out of view", () => {
    expect(scrollToReveal(5, 0, 300, 100)).toBe(300);
    expect(scrollToReveal(1, 400, 300, 100)).toBe(100);
    expect(scrollToReveal(4, 300, 300, 100)).toBeNull();
  });
});
