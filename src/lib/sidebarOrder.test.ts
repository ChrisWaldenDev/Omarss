import { describe, expect, it } from "vitest";

import { currentOrder, moveFeed, moveFolder } from "./sidebarOrder";
import type { FeedNode, Sidebar } from "./types";

function feed(id: number, folderId: number | null): FeedNode {
  return {
    id,
    folderId,
    title: `Feed ${id}`,
    siteUrl: null,
    unreadCount: 0,
    errorCount: 0,
    lastError: null,
    icon: null,
    paused: false,
  };
}

const sidebar: Sidebar = {
  counts: { unread: 0, starred: 0, today: 0 },
  folders: [
    { id: 10, name: "A", unreadCount: 0, feeds: [feed(1, 10), feed(2, 10)] },
    { id: 20, name: "B", unreadCount: 0, feeds: [feed(3, 20)] },
  ],
  feeds: [feed(4, null), feed(5, null)],
  deletedFeeds: null,
};

const ids = (s: Sidebar) => currentOrder(s).feeds.map((f) => `${f.id}@${f.folderId ?? "-"}`);

describe("moveFeed", () => {
  it("reorders within a folder", () => {
    expect(ids(moveFeed(sidebar, 2, { kind: "beforeFeed", feedId: 1 }))).toEqual([
      "2@10",
      "1@10",
      "3@20",
      "4@-",
      "5@-",
    ]);
  });

  it("moves between folders and to the top level", () => {
    expect(ids(moveFeed(sidebar, 1, { kind: "afterFeed", feedId: 3 }))).toEqual([
      "2@10",
      "3@20",
      "1@20",
      "4@-",
      "5@-",
    ]);
    expect(ids(moveFeed(sidebar, 3, { kind: "beforeFeed", feedId: 5 }))).toEqual([
      "1@10",
      "2@10",
      "4@-",
      "3@-",
      "5@-",
    ]);
    expect(ids(moveFeed(sidebar, 4, { kind: "intoFolder", folderId: 20 }))).toEqual([
      "1@10",
      "2@10",
      "3@20",
      "4@20",
      "5@-",
    ]);
    expect(ids(moveFeed(sidebar, 1, { kind: "root" }))).toEqual([
      "2@10",
      "3@20",
      "4@-",
      "5@-",
      "1@-",
    ]);
  });

  it("ignores drops onto itself or unknown feeds", () => {
    expect(moveFeed(sidebar, 1, { kind: "beforeFeed", feedId: 1 })).toBe(sidebar);
    expect(moveFeed(sidebar, 99, { kind: "root" })).toBe(sidebar);
  });
});

describe("moveFolder", () => {
  it("reorders folders", () => {
    expect(
      currentOrder(moveFolder(sidebar, 20, { kind: "beforeFolder", folderId: 10 })).folders,
    ).toEqual([20, 10]);
    expect(moveFolder(sidebar, 10, { kind: "afterFolder", folderId: 10 })).toBe(sidebar);
  });
});
