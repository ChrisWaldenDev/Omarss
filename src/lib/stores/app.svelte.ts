// The app-wide store instances and how they react to backend events.

import { events } from "../types";
import { ArticlesStore } from "./articles.svelte";
import { refresh } from "./refresh.svelte";
import { sidebar } from "./sidebar.svelte";

export const articles = new ArticlesStore(() => sidebar.scheduleReload(0));

/** Subscribes to backend events; returns a function that unsubscribes. */
export async function connectEvents(): Promise<() => void> {
  const unlisten = await Promise.all([
    events.refreshProgress.listen((e) => {
      refresh.onProgress(e.payload);
      if (e.payload.done % 10 === 0) sidebar.scheduleReload(1000);
    }),
    events.refreshDone.listen((e) => {
      refresh.onDone(e.payload);
      sidebar.scheduleReload(0);
    }),
    events.articlesChanged.listen(() => {
      sidebar.scheduleReload(0);
      articles.notifyNewArticles();
    }),
    events.feedError.listen(() => sidebar.scheduleReload()),
  ]);
  return () => unlisten.forEach((stop) => stop());
}
