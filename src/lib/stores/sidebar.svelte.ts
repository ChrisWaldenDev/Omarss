import { api } from "../api";
import type { Sidebar } from "../types";

export class SidebarStore {
  data = $state<Sidebar | null>(null);
  /** Folder IDs the user has collapsed. */
  collapsed = $state<number[]>([]);

  async load(): Promise<void> {
    this.data = await api.getSidebar();
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
