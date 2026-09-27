import { describe, expect, it } from "vitest";

import type { Sidebar } from "./types";
import { sameView, viewTitle } from "./views";

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
        },
      ],
    },
  ],
  feeds: [
    { id: 20, folderId: null, title: "At root", siteUrl: null, unreadCount: 0, errorCount: 0 },
  ],
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
