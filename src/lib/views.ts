import { t } from "./i18n";
import type { Sidebar, View } from "./types";

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
