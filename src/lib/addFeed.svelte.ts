// The "Add feed" flow (SPEC §6.1): address → discovery → pick one if several → preview →
// choose folder → subscribe.

import { api, errorMessage } from "./api";
import type { DiscoveredFeed, FeedPreview } from "./types";

export type AddFeedStep =
  "address" | "discovering" | "choose" | "previewing" | "preview" | "subscribing";

/** A folder choice: an existing folder's id, the top level (`null`), or a new folder. */
export type FolderChoice = number | null | "new";

export class AddFeedFlow {
  step = $state<AddFeedStep>("address");
  address = $state("");
  candidates = $state<DiscoveredFeed[]>([]);
  chosenUrl = $state<string | null>(null);
  preview = $state<FeedPreview | null>(null);
  title = $state("");
  folder = $state<FolderChoice>(null);
  newFolderName = $state("");
  error = $state<string | null>(null);

  busy = $derived(
    this.step === "discovering" || this.step === "previewing" || this.step === "subscribing",
  );

  constructor(address = "") {
    this.address = address;
  }

  /** Discovers feeds at the address; goes straight to the preview if there's only one. */
  async find(): Promise<void> {
    if (!this.address.trim() || this.busy) return;
    this.error = null;
    this.step = "discovering";
    try {
      const found = await api.discoverFeeds(this.address);
      this.candidates = found;
      this.chosenUrl = found[0]?.url ?? null;
      if (found.length === 1) await this.showPreview(found[0].url);
      else this.step = "choose";
    } catch (error) {
      this.error = errorMessage(error);
      this.step = "address";
    }
  }

  async showPreview(url = this.chosenUrl): Promise<void> {
    if (!url) return;
    this.error = null;
    this.step = "previewing";
    try {
      this.preview = await api.previewFeed(url);
      this.title = this.preview.title;
      this.step = "preview";
    } catch (error) {
      this.error = errorMessage(error);
      this.step = this.candidates.length > 1 ? "choose" : "address";
    }
  }

  back(): void {
    this.error = null;
    this.preview = null;
    this.step = this.candidates.length > 1 ? "choose" : "address";
  }

  /** Subscribes; returns the new feed's id, or `null` if it failed (see `error`). */
  async subscribe(): Promise<number | null> {
    if (!this.preview || this.busy) return null;
    this.error = null;
    this.step = "subscribing";
    try {
      let folderId: number | null = null;
      if (this.folder === "new") {
        folderId = await api.createFolder(this.newFolderName);
        this.folder = folderId;
      } else {
        folderId = this.folder;
      }
      return await api.subscribeFeed({
        url: this.preview.url,
        folderId,
        title: this.title.trim() || null,
      });
    } catch (error) {
      this.error = errorMessage(error);
      this.step = "preview";
      return null;
    }
  }
}
