// Drag-and-drop reordering of the sidebar (SPEC §6.1). Pure functions over the sidebar data;
// the result is sent to the backend as a `SidebarOrder`.

import type { FeedNode, Sidebar, SidebarOrder } from "./types";

export type FeedDrop =
  | { kind: "beforeFeed" | "afterFeed"; feedId: number }
  | { kind: "intoFolder"; folderId: number }
  | { kind: "root" };

export type FolderDrop = { kind: "beforeFolder" | "afterFolder"; folderId: number };

/** The current order as the backend expects it: folder feeds in folder order, then root feeds. */
export function currentOrder(sidebar: Sidebar): SidebarOrder {
  return {
    folders: sidebar.folders.map((f) => f.id),
    feeds: [
      ...sidebar.folders.flatMap((f) => f.feeds.map((feed) => ({ id: feed.id, folderId: f.id }))),
      ...sidebar.feeds.map((feed) => ({ id: feed.id, folderId: null })),
    ],
  };
}

export function moveFeed(sidebar: Sidebar, feedId: number, drop: FeedDrop): Sidebar {
  const moving = findFeed(sidebar, feedId);
  if (!moving || (drop.kind !== "root" && drop.kind !== "intoFolder" && drop.feedId === feedId)) {
    return sidebar;
  }
  const without: Sidebar = {
    ...sidebar,
    folders: sidebar.folders.map((f) => ({ ...f, feeds: f.feeds.filter((x) => x.id !== feedId) })),
    feeds: sidebar.feeds.filter((x) => x.id !== feedId),
  };

  if (drop.kind === "root") {
    return { ...without, feeds: [...without.feeds, { ...moving, folderId: null }] };
  }
  if (drop.kind === "intoFolder") {
    return {
      ...without,
      folders: without.folders.map((f) =>
        f.id === drop.folderId ? { ...f, feeds: [...f.feeds, { ...moving, folderId: f.id }] } : f,
      ),
    };
  }

  const insert = (list: FeedNode[], folderId: number | null): FeedNode[] | null => {
    const index = list.findIndex((x) => x.id === drop.feedId);
    if (index < 0) return null;
    const at = drop.kind === "beforeFeed" ? index : index + 1;
    return [...list.slice(0, at), { ...moving, folderId }, ...list.slice(at)];
  };
  const root = insert(without.feeds, null);
  if (root) return { ...without, feeds: root };
  return {
    ...without,
    folders: without.folders.map((f) => ({ ...f, feeds: insert(f.feeds, f.id) ?? f.feeds })),
  };
}

export function moveFolder(sidebar: Sidebar, folderId: number, drop: FolderDrop): Sidebar {
  const moving = sidebar.folders.find((f) => f.id === folderId);
  if (!moving || drop.folderId === folderId) return sidebar;
  const rest = sidebar.folders.filter((f) => f.id !== folderId);
  const index = rest.findIndex((f) => f.id === drop.folderId);
  if (index < 0) return sidebar;
  const at = drop.kind === "beforeFolder" ? index : index + 1;
  return { ...sidebar, folders: [...rest.slice(0, at), moving, ...rest.slice(at)] };
}

export function findFeed(sidebar: Sidebar, feedId: number): FeedNode | undefined {
  return (
    sidebar.feeds.find((f) => f.id === feedId) ??
    sidebar.folders.flatMap((f) => f.feeds).find((f) => f.id === feedId)
  );
}
