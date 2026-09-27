// Windowing maths for the virtualised article list (SPEC §6.2: smooth at 100k rows).

export interface VisibleRange {
  /** First rendered row. */
  start: number;
  /** One past the last rendered row. */
  end: number;
  /** Offset of the first rendered row from the top, in pixels. */
  offset: number;
}

export function visibleRange(
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
  count: number,
  overscan = 6,
): VisibleRange {
  if (count === 0 || rowHeight <= 0) return { start: 0, end: 0, offset: 0 };
  const first = Math.floor(Math.max(0, scrollTop) / rowHeight);
  const visible = Math.ceil(Math.max(0, viewportHeight) / rowHeight) + 1;
  const start = Math.max(0, Math.min(count - 1, first - overscan));
  const end = Math.min(count, first + visible + overscan);
  return { start, end, offset: start * rowHeight };
}

/** Scroll position that brings row `index` fully into view, or `null` if it already is. */
export function scrollToReveal(
  index: number,
  scrollTop: number,
  viewportHeight: number,
  rowHeight: number,
): number | null {
  const top = index * rowHeight;
  const bottom = top + rowHeight;
  if (top < scrollTop) return top;
  if (bottom > scrollTop + viewportHeight) return bottom - viewportHeight;
  return null;
}
