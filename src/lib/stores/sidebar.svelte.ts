import { api } from "../api";
import { findFeed } from "../sidebarOrder";
import type { FeedNode, Sidebar } from "../types";

export class SidebarStore {
  data = $state<Sidebar | null>(null);
  /** Folder IDs the user has collapsed. */
  collapsed = $state<number[]>([]);

  #reloadTimer: ReturnType<typeof setTimeout> | null = null;

  hasFeeds = $derived(
    !!this.data &&
      (this.data.feeds.length > 0 || this.data.folders.some((f) => f.feeds.length > 0)),
  );

  async load(): Promise<void> {
    this.data = await api.getSidebar();
  }

  /** Reloads soon, coalescing bursts (e.g. many feeds finishing a refresh). */
  scheduleReload(delay = 250): void {
    if (this.#reloadTimer) clearTimeout(this.#reloadTimer);
    this.#reloadTimer = setTimeout(() => {
      this.#reloadTimer = null;
      this.load().catch((error) => console.error("sidebar reload failed", error));
    }, delay);
  }

  feed(id: number): FeedNode | undefined {
    if (!this.data) return undefined;
    if (this.data.deletedFeeds?.id === id) return this.data.deletedFeeds;
    return findFeed(this.data, id);
  }

  isCollapsed(folderId: number): boolean {
    return this.collapsed.includes(folderId);
  }

  toggleFolder(folderId: number): void {
    this.collapsed = this.isCollapsed(folderId)
      ? this.collapsed.filter((id) => id !== folderId)
      : [...this.collapsed, folderId];
  }
}

export const sidebar = new SidebarStore();
