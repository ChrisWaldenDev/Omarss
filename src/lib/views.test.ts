import { describe, expect, it } from "vitest";

import type { FeedNode, Sidebar } from "./types";
import { brokenFeeds, isBroken, nextUnreadFeed, sameView, viewTitle } from "./views";

const sidebar: Sidebar = {
  counts: { unread: 0, starred: 0, today: 0 },
  folders: [
    {
      id: 1,
      name: "Tech",
      unreadCount: 0,
      feeds: [
        {
          id: 10,
          folderId: 1,
          title: "In a folder",
          siteUrl: null,
          unreadCount: 0,
          errorCount: 0,
          lastError: null,
          icon: null,
          paused: false,
        },
      ],
    },
  ],
  feeds: [
    {
      id: 20,
      folderId: null,
      title: "At root",
      siteUrl: null,
      unreadCount: 0,
      errorCount: 0,
      lastError: null,
      icon: null,
      paused: false,
    },
  ],
  deletedFeeds: null,
};

describe("sameView", () => {
  it("compares kind and id", () => {
    expect(sameView({ kind: "all" }, { kind: "all" })).toBe(true);
    expect(sameView({ kind: "all" }, { kind: "unread" })).toBe(false);
    expect(sameView({ kind: "feed", id: 1 }, { kind: "feed", id: 1 })).toBe(true);
    expect(sameView({ kind: "feed", id: 1 }, { kind: "feed", id: 2 })).toBe(false);
    expect(sameView({ kind: "feed", id: 1 }, { kind: "folder", id: 1 })).toBe(false);
  });
});

describe("viewTitle", () => {
  it("names smart views, folders and feeds", () => {
    expect(viewTitle({ kind: "starred" }, sidebar)).toBe("Starred");
    expect(viewTitle({ kind: "folder", id: 1 }, sidebar)).toBe("Tech");
    expect(viewTitle({ kind: "feed", id: 10 }, sidebar)).toBe("In a folder");
    expect(viewTitle({ kind: "feed", id: 20 }, sidebar)).toBe("At root");
    expect(viewTitle({ kind: "feed", id: 99 }, sidebar)).toBe("");
  });
});

function feed(id: number, overrides: Partial<FeedNode> = {}): FeedNode {
  return {
    id,
    folderId: null,
    title: `Feed ${id}`,
    siteUrl: null,
    unreadCount: 0,
    errorCount: 0,
    lastError: null,
    icon: null,
    paused: false,
    ...overrides,
  };
}

function withFeeds(folderFeeds: FeedNode[], rootFeeds: FeedNode[]): Sidebar {
  return {
    counts: { unread: 0, starred: 0, today: 0 },
    folders: [{ id: 1, name: "F", unreadCount: 0, feeds: folderFeeds }],
    feeds: rootFeeds,
    deletedFeeds: null,
  };
}

describe("isBroken", () => {
  it("flags repeated failures and feeds paused after an error", () => {
    expect(isBroken(feed(1, { errorCount: 3 }))).toBe(true);
    expect(isBroken(feed(1, { errorCount: 2 }))).toBe(false);
    expect(isBroken(feed(1, { errorCount: 1, paused: true }))).toBe(true);
    expect(isBroken(feed(1, { paused: true }))).toBe(false);
  });

  it("lists broken feeds in sidebar order", () => {
    const tree = withFeeds([feed(1), feed(2, { errorCount: 5 })], [feed(3, { errorCount: 4 })]);
    expect(brokenFeeds(tree).map((f) => f.id)).toEqual([2, 3]);
  });
});

describe("nextUnreadFeed", () => {
  const tree = withFeeds(
    [feed(1, { unreadCount: 2 }), feed(2)],
    [feed(3, { unreadCount: 1 }), feed(4, { unreadCount: 5 })],
  );

  it("walks feeds with unread articles in sidebar order, wrapping around", () => {
    expect(nextUnreadFeed(tree, null, 1)).toBe(1);
    expect(nextUnreadFeed(tree, 1, 1)).toBe(3);
    expect(nextUnreadFeed(tree, 3, 1)).toBe(4);
    expect(nextUnreadFeed(tree, 4, 1)).toBe(1);
    expect(nextUnreadFeed(tree, 1, -1)).toBe(4);
    expect(nextUnreadFeed(tree, null, -1)).toBe(4);
    expect(nextUnreadFeed(tree, 2, -1)).toBe(1);
  });

  it("returns null when no other feed has unread articles", () => {
    expect(nextUnreadFeed(withFeeds([feed(1, { unreadCount: 1 })], []), 1, 1)).toBeNull();
    expect(nextUnreadFeed(null, null, 1)).toBeNull();
  });
});
