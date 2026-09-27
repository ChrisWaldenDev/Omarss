import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Article, ArticleListItem } from "../types";

const api = vi.hoisted(() => ({ listArticles: vi.fn(), getArticle: vi.fn() }));
vi.mock("../api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../api")>()),
  api,
}));

import { ArticlesStore } from "./articles.svelte";

function item(id: number): ArticleListItem {
  return {
    id,
    feedId: 1,
    feedTitle: "Feed",
    title: `Article ${id}`,
    summary: "",
    publishedAt: 0,
    isRead: false,
    isStarred: false,
  };
}

function article(id: number): Article {
  return { ...item(id), url: null, author: null, contentHtml: "<p>hi</p>", enclosures: [] };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

describe("ArticlesStore", () => {
  beforeEach(() => {
    api.listArticles.mockReset().mockResolvedValue([item(1), item(2)]);
    api.getArticle.mockReset().mockImplementation(async (id: number) => article(id));
  });

  it("queries the current view with the unread filter and sort", async () => {
    const store = new ArticlesStore();
    await store.load();
    expect(api.listArticles).toHaveBeenCalledWith({
      view: { kind: "all" },
      unreadOnly: true,
      sort: "newestFirst",
    });
    expect(store.items.map((i) => i.id)).toEqual([1, 2]);

    await store.setSort("oldestFirst");
    await store.setUnreadOnly(false);
    expect(api.listArticles).toHaveBeenLastCalledWith({
      view: { kind: "all" },
      unreadOnly: false,
      sort: "oldestFirst",
    });
  });

  it("switching views closes the open article", async () => {
    const store = new ArticlesStore();
    await store.open(1);
    expect(store.article?.id).toBe(1);

    await store.showView({ kind: "feed", id: 3 });
    expect(store.view).toEqual({ kind: "feed", id: 3 });
    expect(store.selectedId).toBeNull();
    expect(store.article).toBeNull();
  });

  it("ignores a slow article response once another article is opened", async () => {
    const store = new ArticlesStore();
    const slow = deferred<Article>();
    api.getArticle.mockReturnValueOnce(slow.promise);

    const first = store.open(1);
    await store.open(2);
    slow.resolve(article(1));
    await first;

    expect(store.selectedId).toBe(2);
    expect(store.article?.id).toBe(2);
  });

  it("ignores a stale list response", async () => {
    const store = new ArticlesStore();
    const slow = deferred<ArticleListItem[]>();
    api.listArticles.mockReturnValueOnce(slow.promise).mockResolvedValueOnce([item(9)]);

    const first = store.load();
    await store.showView({ kind: "starred" });
    slow.resolve([item(1)]);
    await first;

    expect(store.items.map((i) => i.id)).toEqual([9]);
  });

  it("shows load errors instead of stale items", async () => {
    const store = new ArticlesStore();
    await store.load();
    api.listArticles.mockRejectedValue(new Error("database is locked"));
    await store.load();
    expect(store.items).toEqual([]);
    expect(store.listError).toBe("database is locked");
  });

  it("reports an article that failed to load", async () => {
    const store = new ArticlesStore();
    api.getArticle.mockRejectedValue(new Error("Article 5 not found"));
    await store.open(5);
    expect(store.article).toBeNull();
    expect(store.articleError).toBe("Article 5 not found");
  });
});
