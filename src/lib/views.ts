import { t } from "./i18n";
import type { FeedNode, Sidebar, View } from "./types";

/** Consecutive failures before a feed counts as broken (SPEC §6.1). */
export const ERROR_THRESHOLD = 3;

/**
 * A feed needing attention (SPEC §6.1): three or more failures in a row, or paused after a
 * failure (a `410 Gone` pauses a feed at its first error, SPEC §7.1).
 */
export function isBroken(feed: FeedNode): boolean {
  return feed.errorCount >= ERROR_THRESHOLD || (feed.paused && feed.errorCount > 0);
}

/** Every feed in sidebar order: folders' feeds first, then top-level feeds. */
export function feedsInOrder(sidebar: Sidebar | null): FeedNode[] {
  if (!sidebar) return [];
  return [...sidebar.folders.flatMap((f) => f.feeds), ...sidebar.feeds];
}

export function brokenFeeds(sidebar: Sidebar | null): FeedNode[] {
  return feedsInOrder(sidebar).filter(isBroken);
}

/**
 * The next (`delta` 1) or previous (-1) feed with unread articles after `currentFeedId`, in
 * sidebar order and wrapping around (`J`/`K`, SPEC §6.7). `null` if no other feed has any.
 */
export function nextUnreadFeed(
  sidebar: Sidebar | null,
  currentFeedId: number | null,
  delta: 1 | -1,
): number | null {
  const feeds = feedsInOrder(sidebar);
  if (feeds.length === 0) return null;
  const start = feeds.findIndex((f) => f.id === currentFeedId);
  for (let step = 1; step <= feeds.length; step++) {
    const from = start === -1 ? (delta > 0 ? -1 : 0) : start;
    const index = (((from + delta * step) % feeds.length) + feeds.length) % feeds.length;
    const feed = feeds[index];
    if (feed.unreadCount > 0 && feed.id !== currentFeedId) return feed.id;
  }
  return null;
}

export function sameView(a: View, b: View): boolean {
  if (a.kind !== b.kind) return false;
  if ((a.kind === "feed" || a.kind === "folder") && (b.kind === "feed" || b.kind === "folder")) {
    return a.id === b.id;
  }
  return true;
}

/** Heading for the article list: a smart view name, or the folder/feed title. */
export function viewTitle(view: View, sidebar: Sidebar | null): string {
  switch (view.kind) {
    case "all":
      return t("view.all");
    case "unread":
      return t("view.unread");
    case "starred":
      return t("view.starred");
    case "today":
      return t("view.today");
    case "folder":
      return sidebar?.folders.find((f) => f.id === view.id)?.name ?? "";
    case "feed": {
      const feeds = [
        ...(sidebar?.feeds ?? []),
        ...(sidebar?.folders.flatMap((f) => f.feeds) ?? []),
      ];
      return feeds.find((f) => f.id === view.id)?.title ?? "";
    }
  }
}
