import { api, errorMessage } from "../api";
import { t } from "../i18n";
import type {
  Article,
  ArticleCursor,
  ArticleListItem,
  MarkAllReadResult,
  MarkReadMode,
  OlderThan,
  SortOrder,
  View,
} from "../types";
import { toasts } from "./toasts.svelte";

export const PAGE_SIZE = 100;

/** When opened articles count as read (SPEC §6.2); set from the settings. */
export interface MarkReadPolicy {
  mode: MarkReadMode;
  delaySeconds: number;
}

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

  /** The list's cursor. Usually the open article; `n`/`p` move it without opening. */
  selectedId = $state<number | null>(null);
  article = $state<Article | null>(null);
  articleError = $state<string | null>(null);
  /** The "Broken feeds" page is showing instead of an article list (SPEC §6.1). */
  brokenFeedsOpen = $state(false);

  markRead: MarkReadPolicy = { mode: "onOpen", delaySeconds: 3 };

  // Responses for superseded requests are dropped, so fast clicking never shows stale data.
  #listRequest = 0;
  #articleRequest = 0;
  #markReadTimer: ReturnType<typeof setTimeout> | null = null;

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
    this.brokenFeedsOpen = false;
    this.close();
    await this.load();
  }

  showBrokenFeeds(): void {
    this.brokenFeedsOpen = true;
    this.close();
  }

  async setUnreadOnly(unreadOnly: boolean): Promise<void> {
    this.unreadOnly = unreadOnly;
    await this.load();
  }

  async setSort(sort: SortOrder): Promise<void> {
    this.sort = sort;
    await this.load();
  }

  /**
   * Opens an article in the reader and marks it read: at once, or after the configured delay
   * (SPEC §6.2).
   */
  async open(id: number): Promise<void> {
    const request = ++this.#articleRequest;
    this.#cancelMarkRead();
    this.selectedId = id;
    this.articleError = null;
    try {
      const article = await api.getArticle(id);
      if (request !== this.#articleRequest) return;
      this.article = article;
      if (!article.isRead) {
        if (this.markRead.mode === "afterDelay") {
          this.#markReadTimer = setTimeout(() => {
            this.#markReadTimer = null;
            if (this.article?.id === id && !this.article.isRead) void this.#markOpenedRead(id);
          }, this.markRead.delaySeconds * 1000);
        } else {
          await this.#markOpenedRead(id);
        }
      }
    } catch (error) {
      if (request !== this.#articleRequest) return;
      this.article = null;
      this.articleError = errorMessage(error);
    }
  }

  /** A failed mark-read has already been rolled back by `setRead`; the article stays open and
   * the failure is reported without blanking the reader. */
  async #markOpenedRead(id: number): Promise<void> {
    try {
      await this.setRead(id, true);
    } catch (error) {
      toasts.show(t("error.action", { message: errorMessage(error) }));
    }
  }

  #cancelMarkRead(): void {
    if (this.#markReadTimer) clearTimeout(this.#markReadTimer);
    this.#markReadTimer = null;
  }

  close(): void {
    this.#articleRequest += 1;
    this.#cancelMarkRead();
    this.selectedId = null;
    this.article = null;
    this.articleError = null;
  }

  /** Moves the list cursor without opening anything (`n`/`p`). */
  select(id: number): void {
    this.selectedId = id;
  }

  /**
   * Moves the cursor to the next (`delta` 1) or previous (-1) article, opening it if `open`
   * (`j`/`k`). Past the loaded rows it loads the next page first.
   */
  async move(delta: 1 | -1, open: boolean): Promise<void> {
    if (this.items.length === 0) return;
    const index = this.items.findIndex((i) => i.id === this.selectedId);
    const target = index === -1 ? (delta > 0 ? 0 : this.items.length - 1) : index + delta;
    if (target >= this.items.length && this.next) await this.loadMore();
    const item = this.items[target];
    if (!item || target < 0) return;
    if (open) await this.open(item.id);
    else this.select(item.id);
  }

  /** Opens the article under the cursor, if it isn't already open (`o`/`Enter`). */
  async openSelected(): Promise<void> {
    if (this.selectedId !== null && this.article?.id !== this.selectedId) {
      await this.open(this.selectedId);
    }
  }

  async toggleRead(id: number): Promise<void> {
    const current = this.article?.id === id ? this.article : this.items.find((i) => i.id === id);
    if (!current) return;
    this.#cancelMarkRead();
    await this.setRead(id, !current.isRead);
  }

  /**
   * Marks every unread article in the current view read, optionally only older ones
   * (SPEC §6.2). The result's token can undo it with `undoMarkAllRead`.
   */
  async markAllRead(olderThan: OlderThan | null = null): Promise<MarkAllReadResult> {
    const result = await api.markAllRead(this.view, olderThan);
    if (result.count > 0) {
      this.onCountsChanged();
      await Promise.all([this.load(), this.#syncOpenArticle()]);
    }
    return result;
  }

  async undoMarkAllRead(token: number): Promise<void> {
    await api.undoMarkAllRead(token);
    this.onCountsChanged();
    await Promise.all([this.load(), this.#syncOpenArticle()]);
  }

  /** Marks articles that scrolled past the top of the list read ("on scroll" mode). */
  async markScrolledPast(ids: number[]): Promise<void> {
    const undo = ids.map((id) => this.#patch(id, { isRead: true }));
    try {
      await api.setArticlesRead(ids, true);
      this.onCountsChanged();
    } catch (error) {
      undo.forEach((u) => u());
      throw error;
    }
  }

  /** Picks up read-state changes made to the open article by a bulk action. */
  async #syncOpenArticle(): Promise<void> {
    const open = this.article;
    if (!open) return;
    try {
      const fresh = await api.getArticle(open.id);
      if (this.article?.id === open.id) {
        this.article = { ...this.article, isRead: fresh.isRead, isStarred: fresh.isStarred };
      }
    } catch {
      // The article may have been deleted meanwhile; the reader keeps what it shows.
    }
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
