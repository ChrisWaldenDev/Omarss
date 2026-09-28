// Lets keyboard shortcuts scroll the reader pane (Space / Shift+Space, SPEC §6.7).

let scroller: HTMLElement | null = null;

export function registerReaderScroller(element: HTMLElement | null): void {
  scroller = element;
}

type Scrollable = Pick<HTMLElement, "scrollTop" | "clientHeight" | "scrollHeight" | "scrollBy">;

/**
 * Scrolls most of a screen down (1) or up (-1). Returns false when there's nothing to scroll
 * that way, so Space can move on to the next article at the end.
 */
export function scrollReader(direction: 1 | -1, element: Scrollable | null = scroller): boolean {
  if (!element) return false;
  const atEnd =
    direction > 0
      ? element.scrollTop + element.clientHeight >= element.scrollHeight - 2
      : element.scrollTop <= 0;
  if (atEnd) return false;
  const smooth = !globalThis.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
  element.scrollBy({
    top: direction * Math.max(40, element.clientHeight * 0.85),
    behavior: smooth ? "smooth" : "auto",
  });
  return true;
}
