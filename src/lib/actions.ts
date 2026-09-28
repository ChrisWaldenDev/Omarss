// What keyboard shortcuts, menus and buttons do (SPEC §6.2, §6.4, §6.7). Failures are shown
// as toasts; nothing here throws.

import { api, errorMessage } from "./api";
import { t } from "./i18n";
import type { ActionId } from "./keyboard";
import { scrollReader } from "./readerScroll";
import { copyText, mailtoLink, markdownLink } from "./share";
import { articles } from "./stores/app.svelte";
import { dialogs } from "./stores/dialogs.svelte";
import { refresh } from "./stores/refresh.svelte";
import { sidebar } from "./stores/sidebar.svelte";
import { toasts } from "./stores/toasts.svelte";
import type { OlderThan, View } from "./types";
import { nextUnreadFeed } from "./views";

/** How long "Mark all as read" can be undone (SPEC §6.2). */
export const UNDO_MS = 10_000;

function report(error: unknown): void {
  toasts.show(t("error.action", { message: errorMessage(error) }));
}

/** Runs `task`, showing a toast if it fails. */
export async function attempt(task: () => Promise<unknown>): Promise<void> {
  try {
    await task();
  } catch (error) {
    report(error);
  }
}

/** Marks the current view read, with an Undo toast (SPEC §6.2). */
export async function markAllRead(olderThan: OlderThan | null = null): Promise<void> {
  await attempt(async () => {
    const result = await articles.markAllRead(olderThan);
    if (result.count === 0) {
      toasts.info(t("markAll.nothing"));
      return;
    }
    const token = result.undoToken;
    toasts.info(t("markAll.done", { count: result.count }), {
      timeoutMs: UNDO_MS,
      action:
        token === null
          ? undefined
          : {
              label: t("markAll.undo"),
              run: () => attempt(() => articles.undoMarkAllRead(token)),
            },
    });
  });
}

export async function importOpml(): Promise<void> {
  await attempt(async () => {
    const summary = await api.importOpml();
    if (!summary) return;
    await sidebar.load();
    toasts.info(t("opml.imported", { ...summary }), { timeoutMs: 12_000 });
  });
}

export async function exportOpml(): Promise<void> {
  await attempt(async () => {
    const path = await api.exportOpml();
    if (path) toasts.info(t("opml.exported", { path }));
  });
}

/** The article the reader shows, else the one under the list cursor. */
function targetArticle(): number | null {
  return articles.article?.id ?? articles.selectedId;
}

async function articleUrl(id: number): Promise<{ title: string; url: string } | null> {
  const article = articles.article?.id === id ? articles.article : await api.getArticle(id);
  return article.url ? { title: article.title, url: article.url } : null;
}

export async function openInBrowser(id: number | null = targetArticle()): Promise<void> {
  if (id === null) return;
  await attempt(async () => {
    const link = await articleUrl(id);
    if (link) await api.openExternal(link.url);
  });
}

export type ShareKind = "link" | "markdown" | "email";

export async function share(id: number, kind: ShareKind): Promise<void> {
  await attempt(async () => {
    const link = await articleUrl(id);
    if (!link) return;
    if (kind === "email") {
      await api.openExternal(mailtoLink(link.title, link.url));
      return;
    }
    await copyText(kind === "markdown" ? markdownLink(link.title, link.url) : link.url);
    toasts.info(t("share.copied"), { timeoutMs: 2500 });
  });
}

function currentFeedId(): number | null {
  return articles.view.kind === "feed" ? articles.view.id : null;
}

async function showView(view: View): Promise<void> {
  await attempt(() => articles.showView(view));
}

export async function runAction(id: ActionId): Promise<void> {
  switch (id) {
    case "nextArticle":
      return attempt(() => articles.move(1, true));
    case "prevArticle":
      return attempt(() => articles.move(-1, true));
    case "nextItem":
      return attempt(() => articles.move(1, false));
    case "prevItem":
      return attempt(() => articles.move(-1, false));
    case "nextUnreadFeed":
    case "prevUnreadFeed": {
      const feed = nextUnreadFeed(sidebar.data, currentFeedId(), id === "nextUnreadFeed" ? 1 : -1);
      if (feed !== null) await showView({ kind: "feed", id: feed });
      return;
    }
    case "openSelected":
      return attempt(() => articles.openSelected());
    case "openInBrowser":
      return openInBrowser();
    case "toggleRead": {
      const target = targetArticle();
      if (target !== null) await attempt(() => articles.toggleRead(target));
      return;
    }
    case "toggleStar": {
      const target = targetArticle();
      if (target !== null) await attempt(() => articles.toggleStar(target));
      return;
    }
    case "markAllRead":
      return markAllRead();
    case "refreshCurrent": {
      const view = articles.view;
      const target =
        view.kind === "feed" || view.kind === "folder"
          ? { kind: view.kind, id: view.id }
          : { kind: "all" as const };
      return attempt(() => refresh.request(target));
    }
    case "refreshAll":
      return attempt(() => refresh.request({ kind: "all" }));
    case "goAll":
      return showView({ kind: "all" });
    case "goUnread":
      return showView({ kind: "unread" });
    case "goStarred":
      return showView({ kind: "starred" });
    case "scrollDown":
      if (!articles.article || !scrollReader(1)) await attempt(() => articles.move(1, true));
      return;
    case "scrollUp":
      scrollReader(-1);
      return;
    case "openSettings":
      dialogs.open({ kind: "settings" });
      return;
    case "showShortcuts":
      dialogs.open({ kind: "shortcuts" });
      return;
  }
}

export async function openExternalLink(url: string): Promise<void> {
  await attempt(() => api.openExternal(url));
}
