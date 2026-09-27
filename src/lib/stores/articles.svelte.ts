import { api, errorMessage } from "../api";
import type { Article, ArticleListItem, SortOrder, View } from "../types";

/** The current view, its article list, and the article open in the reader. */
export class ArticlesStore {
  view = $state<View>({ kind: "all" });
  unreadOnly = $state(true);
  sort = $state<SortOrder>("newestFirst");

  items = $state<ArticleListItem[]>([]);
  listError = $state<string | null>(null);

  selectedId = $state<number | null>(null);
  article = $state<Article | null>(null);
  articleError = $state<string | null>(null);

  // Responses for superseded requests are dropped, so fast clicking never shows stale data.
  #listRequest = 0;
  #articleRequest = 0;

  async load(): Promise<void> {
    const request = ++this.#listRequest;
    try {
      const items = await api.listArticles({
        view: this.view,
        unreadOnly: this.unreadOnly,
        sort: this.sort,
      });
      if (request !== this.#listRequest) return;
      this.items = items;
      this.listError = null;
    } catch (error) {
      if (request !== this.#listRequest) return;
      this.items = [];
      this.listError = errorMessage(error);
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

  async open(id: number): Promise<void> {
    const request = ++this.#articleRequest;
    this.selectedId = id;
    this.articleError = null;
    try {
      const article = await api.getArticle(id);
      if (request === this.#articleRequest) this.article = article;
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
}

export const articles = new ArticlesStore();
