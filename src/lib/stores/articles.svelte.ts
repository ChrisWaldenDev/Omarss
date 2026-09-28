import { api, errorMessage } from "../api";
import { t } from "../i18n";
import type { Article, ArticleCursor, ArticleListItem, SortOrder, View } from "../types";
import { toasts } from "./toasts.svelte";

export const PAGE_SIZE = 100;

/** The current view, its (paged) article list, and the article open in the reader. */
export class ArticlesStore {
  view = $state<View>({ kind: "all" });
  unreadOnly = $state(true);
  sort = $state<SortOrder>("newestFirst");

  items = $state<ArticleListItem[]>([]);
  next = $state<ArticleCursor | null>(null);
  loading = $state(false);
  loadingMore = $state(false);
  listError = $state<string | null>(null);
  /** New articles arrived while a list was showing; shown as a "show new" button. */
  hasNewArticles = $state(false);

  selectedId = $state<number | null>(null);
  article = $state<Article | null>(null);
  articleError = $state<string | null>(null);

  // Responses for superseded requests are dropped, so fast clicking never shows stale data.
  #listRequest = 0;
  #articleRequest = 0;

  /** `onCountsChanged` runs after read/star changes so unread counts can refresh. */
  constructor(private readonly onCountsChanged: () => void = () => {}) {}

  #query() {
    return { view: this.view, unreadOnly: this.unreadOnly, sort: this.sort };
  }

  async load(): Promise<void> {
    const request = ++this.#listRequest;
    this.loading = true;
    this.loadingMore = false;
    // Cleared now, not when the page arrives: articles announced while the request is in
    // flight may not be in it, so their notice must survive.
    this.hasNewArticles = false;
    try {
      const page = await api.listArticles(this.#query(), null, PAGE_SIZE);
      if (request !== this.#listRequest) return;
      this.items = page.items;
      this.next = page.next;
      this.listError = null;
    } catch (error) {
      if (request !== this.#listRequest) return;
      this.items = [];
      this.next = null;
      this.listError = errorMessage(error);
    } finally {
      if (request === this.#listRequest) this.loading = false;
    }
  }

  /** Loads the next page, if there is one (infinite scrolling). */
  async loadMore(): Promise<void> {
    if (!this.next || this.loadingMore || this.loading) return;
    const request = this.#listRequest;
    this.loadingMore = true;
    try {
      const page = await api.listArticles(this.#query(), this.next, PAGE_SIZE);
      if (request !== this.#listRequest) return;
      // eslint-disable-next-line svelte/prefer-svelte-reactivity -- a temporary lookup, not state
      const known = new Set(this.items.map((i) => i.id));
      this.items = [...this.items, ...page.items.filter((i) => !known.has(i.id))];
      this.next = page.next;
    } catch (error) {
      if (request === this.#listRequest) this.listError = errorMessage(error);
    } finally {
      if (request === this.#listRequest) this.loadingMore = false;
    }
  }

  async showView(view: View): Promise<void> {
    this.view = view;
    this.close();
    await this.load();
  }

  async setUnreadOnly(unreadOnly: boolean): Promise<void> {
    this.unreadOnly = unreadOnly;
    await this.load();
  }

  async setSort(sort: SortOrder): Promise<void> {
    this.sort = sort;
    await this.load();
  }

  /** Opens an article in the reader and marks it read (SPEC §6.2: mark read on open). */
  async open(id: number): Promise<void> {
    const request = ++this.#articleRequest;
    this.selectedId = id;
    this.articleError = null;
    try {
      const article = await api.getArticle(id);
      if (request !== this.#articleRequest) return;
      this.article = article;
      if (!article.isRead) {
        // A failed mark-read has already been rolled back by `setRead`; the article stays
        // open and the failure is reported without blanking the reader.
        try {
          await this.setRead(id, true);
        } catch (error) {
          toasts.show(t("error.action", { message: errorMessage(error) }));
        }
      }
    } catch (error) {
      if (request !== this.#articleRequest) return;
      this.article = null;
      this.articleError = errorMessage(error);
    }
  }

  close(): void {
    this.#articleRequest += 1;
    this.selectedId = null;
    this.article = null;
    this.articleError = null;
  }

  async setRead(id: number, read: boolean): Promise<void> {
    const undo = this.#patch(id, { isRead: read });
    try {
      await api.setArticlesRead([id], read);
      this.onCountsChanged();
    } catch (error) {
      undo();
      throw error;
    }
  }

  async toggleStar(id: number): Promise<void> {
    const current = this.article?.id === id ? this.article : this.items.find((i) => i.id === id);
    if (!current) return;
    const starred = !current.isStarred;
    const undo = this.#patch(id, { isStarred: starred });
    try {
      await api.setArticleStarred(id, starred);
      this.onCountsChanged();
    } catch (error) {
      undo();
      throw error;
    }
  }

  /** Called when a background refresh added articles. */
  notifyNewArticles(): void {
    if (this.items.length === 0 && !this.loading) void this.load();
    else this.hasNewArticles = true;
  }

  /** Applies `changes` to the list item and open article; returns a function undoing it. */
  #patch(id: number, changes: Partial<Pick<ArticleListItem, "isRead" | "isStarred">>): () => void {
    const item = this.items.find((i) => i.id === id);
    const before = item ? { isRead: item.isRead, isStarred: item.isStarred } : null;
    const articleBefore =
      this.article?.id === id
        ? { isRead: this.article.isRead, isStarred: this.article.isStarred }
        : null;
    if (item) Object.assign(item, changes);
    if (this.article?.id === id) this.article = { ...this.article, ...changes };
    return () => {
      if (item && before) Object.assign(item, before);
      if (articleBefore && this.article?.id === id) {
        this.article = { ...this.article, ...articleBefore };
      }
    };
  }
}
