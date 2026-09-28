import type { MessageKey } from "./i18n";
import type { ActionId } from "./keyboard";

/** Shortcuts grouped for the cheatsheet and the settings page. */
export const SHORTCUT_GROUPS: { title: MessageKey; actions: ActionId[] }[] = [
  {
    title: "shortcuts.group.navigate",
    actions: [
      "nextArticle",
      "prevArticle",
      "nextItem",
      "prevItem",
      "openSelected",
      "nextUnreadFeed",
      "prevUnreadFeed",
      "goAll",
      "goUnread",
      "goStarred",
    ],
  },
  {
    title: "shortcuts.group.articles",
    actions: ["toggleRead", "toggleStar", "openInBrowser", "markAllRead", "scrollDown", "scrollUp"],
  },
  {
    title: "shortcuts.group.app",
    actions: ["refreshCurrent", "refreshAll", "openSettings", "showShortcuts"],
  },
];
