import { describe, expect, it, vi } from "vitest";

import { mailtoLink, markdownLink } from "./share";
import { scrollReader } from "./readerScroll";

describe("share links", () => {
  it("escapes Markdown", () => {
    expect(markdownLink("Why [x] matters", "https://a.example/p_(1)")).toBe(
      "[Why \\[x\\] matters](https://a.example/p_%281%29)",
    );
  });

  it("builds a mailto link", () => {
    expect(mailtoLink("A & B", "https://a.example/?q=1&r=2")).toBe(
      "mailto:?subject=A%20%26%20B&body=https%3A%2F%2Fa.example%2F%3Fq%3D1%26r%3D2",
    );
  });
});

describe("scrollReader", () => {
  const pane = (scrollTop: number) => ({
    scrollTop,
    clientHeight: 500,
    scrollHeight: 2000,
    scrollBy: vi.fn(),
  });

  it("scrolls until the end, then reports there's nothing left", () => {
    const middle = pane(600);
    expect(scrollReader(1, middle)).toBe(true);
    expect(middle.scrollBy).toHaveBeenCalledWith(expect.objectContaining({ top: 425 }));
    expect(scrollReader(1, pane(1500))).toBe(false);
    expect(scrollReader(-1, pane(0))).toBe(false);
    expect(scrollReader(-1, pane(600))).toBe(true);
    expect(scrollReader(1, null)).toBe(false);
  });
});
