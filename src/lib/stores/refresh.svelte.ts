import { api } from "../api";
import type { RefreshDone, RefreshProgress, RefreshStatus, RefreshTarget } from "../types";

/** Background refresh progress (SPEC §7.2), fed by `refresh:*` events. */
export class RefreshStore {
  running = $state(false);
  done = $state(0);
  total = $state(0);
  offline = $state(false);
  lastFinishedAt = $state<number | null>(null);

  apply(status: RefreshStatus): void {
    this.running = status.running;
    this.done = status.done;
    this.total = status.total;
    this.offline = status.offline;
    this.lastFinishedAt = status.lastFinishedAt;
  }

  async load(): Promise<void> {
    this.apply(await api.getRefreshStatus());
  }

  onProgress(progress: RefreshProgress): void {
    this.running = progress.done < progress.total;
    this.done = progress.done;
    this.total = progress.total;
  }

  onDone(result: RefreshDone): void {
    this.running = false;
    this.offline = result.offline;
    this.lastFinishedAt = Math.floor(Date.now() / 1000);
  }

  async request(target: RefreshTarget): Promise<void> {
    this.running = true;
    await api.refresh(target);
  }
}

export const refresh = new RefreshStore();
