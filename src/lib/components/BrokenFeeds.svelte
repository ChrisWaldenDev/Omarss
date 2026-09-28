<script lang="ts">
  // "Broken feeds" (SPEC §6.1): feeds that keep failing, with their last error and ways to fix
  // them.
  import { attempt } from "../actions";
  import { t } from "../i18n";
  import { dialogs } from "../stores/dialogs.svelte";
  import { refresh } from "../stores/refresh.svelte";
  import { sidebar } from "../stores/sidebar.svelte";
  import type { FeedNode } from "../types";
  import { brokenFeeds } from "../views";
  import FeedIcon from "./FeedIcon.svelte";
  import Icon from "./Icon.svelte";

  const feeds = $derived(brokenFeeds(sidebar.data));
  let retrying = $state<number[]>([]);

  async function retry(feed: FeedNode) {
    retrying = [...retrying, feed.id];
    await attempt(() => refresh.request({ kind: "feed", id: feed.id }));
  }

  // A retry is done once the refresh that ran it finishes.
  $effect(() => {
    if (!refresh.running) retrying = [];
  });
</script>

<section class="broken" aria-labelledby="broken-title">
  <header class="header">
    <h1 id="broken-title">{t("broken.title")}</h1>
    <p>{t("broken.intro")}</p>
  </header>
  {#if feeds.length === 0}
    <p class="empty">{t("broken.none")}</p>
  {:else}
    <ul>
      {#each feeds as feed (feed.id)}
        <li>
          <div class="name">
            <FeedIcon title={feed.title} icon={feed.icon} />
            <strong>{feed.title}</strong>
            {#if feed.paused}
              <span class="tag">{t("broken.paused")}</span>
            {/if}
          </div>
          <p class="error">
            <Icon name="warning" size={14} />
            <span>
              {#if feed.lastError}
                {feed.lastError}
                <span class="count">· {t("sidebar.feedErrors", { count: feed.errorCount })}</span>
              {:else}
                {t("sidebar.feedErrors", { count: feed.errorCount })}
              {/if}
            </span>
          </p>
          <div class="actions">
            <button
              class="button"
              onclick={() => retry(feed)}
              disabled={retrying.includes(feed.id)}
            >
              <Icon name="refresh" size={14} />
              {retrying.includes(feed.id) ? t("broken.retrying") : t("broken.retry")}
            </button>
            <button
              class="button"
              onclick={() => dialogs.open({ kind: "editFeed", feedId: feed.id, focusUrl: true })}
            >
              {t("broken.editUrl")}
            </button>
          </div>
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .broken {
    min-width: 0;
    overflow-y: auto;
    padding: 24px 32px 48px;
    background: var(--bg);
  }

  .header h1 {
    margin: 0 0 4px;
    font-size: 1.3rem;
  }

  .header p,
  .empty {
    margin: 0 0 20px;
    color: var(--text-muted);
  }

  ul {
    display: flex;
    flex-direction: column;
    gap: 12px;
    max-width: 48rem;
    margin: 0;
    padding: 0;
    list-style: none;
  }

  li {
    padding: 14px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-list);
  }

  .name {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .tag {
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-muted);
    font-size: 0.75rem;
  }

  .error {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    margin: 8px 0 12px;
    color: var(--warning);
    font-size: 0.88rem;
    overflow-wrap: anywhere;
  }

  .error :global(svg) {
    margin-top: 2px;
  }

  .count {
    color: var(--text-muted);
  }

  .actions {
    display: flex;
    gap: 8px;
  }
</style>
