import { beforeEach, describe, expect, it, vi } from "vitest";

import type { Article, ArticleListItem, ArticlePage } from "../types";

const api = vi.hoisted(() => ({
  listArticles: vi.fn(),
  getArticle: vi.fn(),
  setArticlesRead: vi.fn(),
  setArticleStarred: vi.fn(),
}));
vi.mock("../api", async (importOriginal) => ({
  ...(await importOriginal<typeof import("../api")>()),
  api,
}));
const toasts = vi.hoisted(() => ({ show: vi.fn() }));
vi.mock("./toasts.svelte", () => ({ toasts }));

import { ArticlesStore, PAGE_SIZE } from "./articles.svelte";

function item(id: number, overrides: Partial<ArticleListItem> = {}): ArticleListItem {
  return {
    id,
    feedId: 1,
    feedTitle: "Feed",
    title: `Article ${id}`,
    summary: "",
    publishedAt: 1000 - id,
    isRead: false,
    isStarred: false,
    ...overrides,
  };
}

function article(id: number, overrides: Partial<Article> = {}): Article {
  return {
    id,
    feedId: 1,
    feedTitle: "Feed",
    title: `Article ${id}`,
    url: null,
    author: null,
    contentHtml: "<p>hi</p>",
    publishedAt: 0,
    isRead: false,
    isStarred: false,
    enclosures: [],
    ...overrides,
  };
}

function page(items: ArticleListItem[], next: ArticlePage["next"] = null): ArticlePage {
  return { items, next };
}

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => (resolve = r));
  return { promise, resolve };
}

describe("ArticlesStore", () => {
  let countsChanged: ReturnType<typeof vi.fn<() => void>>;
  let store: ArticlesStore;

  beforeEach(() => {
    for (const fn of Object.values(api)) fn.mockReset();
    toasts.show.mockReset();
    api.listArticles.mockResolvedValue(page([item(1), item(2)]));
    api.getArticle.mockImplementation(async (id: number) => article(id));
    api.setArticlesRead.mockResolvedValue(1);
    api.setArticleStarred.mockResolvedValue(null);
    countsChanged = vi.fn<() => void>();
    store = new ArticlesStore(countsChanged);
  });

  it("loads the first page of the current view", async () => {
    await store.load();
    expect(api.listArticles).toHaveBeenCalledWith(
      { view: { kind: "all" }, unreadOnly: true, sort: "newestFirst" },
      null,
      PAGE_SIZE,
    );
    expect(store.items.map((i) => i.id)).toEqual([1, 2]);
    expect(store.loading).toBe(false);
  });

  it("appends further pages using the cursor", async () => {
    const cursor = { publishedAt: 998, id: 2 };
    api.listArticles
      .mockResolvedValueOnce(page([item(1), item(2)], cursor))
      .mockResolvedValueOnce(page([item(2), item(3)]));
    await store.load();
    await store.loadMore();
    expect(api.listArticles).toHaveBeenLastCalledWith(expect.anything(), cursor, PAGE_SIZE);
    expect(store.items.map((i) => i.id)).toEqual([1, 2, 3]);
    expect(store.next).toBeNull();
    await store.loadMore();
    expect(api.listArticles).toHaveBeenCalledTimes(2);
  });

  it("drops a page that arrives after the view changed", async () => {
    const slow = deferred<ArticlePage>();
    api.listArticles
      .mockResolvedValueOnce(page([item(1)], { publishedAt: 1, id: 1 }))
      .mockReturnValueOnce(slow.promise)
      .mockResolvedValueOnce(page([item(9)]));
    await store.load();
    const more = store.loadMore();
    await store.showView({ kind: "starred" });
    slow.resolve(page([item(5)]));
    await more;
    expect(store.items.map((i) => i.id)).toEqual([9]);
  });

  it("marks an article read when it's opened", async () => {
    await store.load();
    await store.open(1);
    expect(store.article?.id).toBe(1);
    expect(api.setArticlesRead).toHaveBeenCalledWith([1], true);
    expect(store.items[0].isRead).toBe(true);
    expect(store.article?.isRead).toBe(true);
    expect(countsChanged).toHaveBeenCalled();

    api.getArticle.mockResolvedValueOnce(article(2, { isRead: true }));
    api.setArticlesRead.mockClear();
    await store.open(2);
    expect(api.setArticlesRead).not.toHaveBeenCalled();
  });

  it("keeps the article open when marking it read fails", async () => {
    await store.load();
    api.setArticlesRead.mockRejectedValueOnce(new Error("database is locked"));
    await store.open(1);
    expect(store.article?.id).toBe(1);
    expect(store.articleError).toBeNull();
    expect(store.article?.isRead).toBe(false);
    expect(store.items[0].isRead).toBe(false);
    expect(toasts.show).toHaveBeenCalledWith(expect.stringContaining("database is locked"));
  });

  it("stars optimistically and rolls back if saving fails", async () => {
    await store.load();
    await store.open(1);
    await store.toggleStar(1);
    expect(api.setArticleStarred).toHaveBeenCalledWith(1, true);
    expect(store.items[0].isStarred && store.article?.isStarred).toBe(true);

    api.setArticleStarred.mockRejectedValueOnce(new Error("disk full"));
    await expect(store.toggleStar(1)).rejects.toThrow("disk full");
    expect(store.items[0].isStarred).toBe(true);
    expect(store.article?.isStarred).toBe(true);
  });

  it("can mark an article unread again", async () => {
    await store.load();
    await store.open(1);
    await store.setRead(1, false);
    expect(api.setArticlesRead).toHaveBeenLastCalledWith([1], false);
    expect(store.items[0].isRead).toBe(false);
  });

  it("ignores a slow article once another is opened", async () => {
    const slow = deferred<Article>();
    api.getArticle.mockReturnValueOnce(slow.promise);
    const first = store.open(1);
    await store.open(2);
    slow.resolve(article(1));
    await first;
    expect(store.selectedId).toBe(2);
    expect(store.article?.id).toBe(2);
  });

  it("reports load errors", async () => {
    api.listArticles.mockRejectedValue(new Error("database is locked"));
    await store.load();
    expect(store.items).toEqual([]);
    expect(store.listError).toBe("database is locked");
    api.getArticle.mockRejectedValue(new Error("Article 5 not found"));
    await store.open(5);
    expect(store.articleError).toBe("Article 5 not found");
  });

  it("offers new articles instead of reshuffling a list being read", async () => {
    await store.load();
    store.notifyNewArticles();
    expect(store.hasNewArticles).toBe(true);
    expect(api.listArticles).toHaveBeenCalledTimes(1);

    const empty = new ArticlesStore();
    api.listArticles.mockResolvedValueOnce(page([]));
    await empty.load();
    empty.notifyNewArticles();
    expect(api.listArticles).toHaveBeenCalledTimes(3);
  });

  it("keeps a new-articles notice that arrives while the list is loading", async () => {
    await store.load();
    const slow = deferred<ArticlePage>();
    api.listArticles.mockReturnValueOnce(slow.promise);
    const loading = store.load();
    store.notifyNewArticles();
    slow.resolve(page([item(1), item(2)]));
    await loading;
    expect(store.hasNewArticles).toBe(true);

    // A load that starts afterwards clears it.
    await store.load();
    expect(store.hasNewArticles).toBe(false);
  });
});
