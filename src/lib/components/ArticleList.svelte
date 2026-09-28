<script lang="ts">
  import { relativeTime } from "../format";
  import { t } from "../i18n";
  import { articles } from "../stores/app.svelte";
  import { clock } from "../stores/clock.svelte";
  import { dialogs } from "../stores/dialogs.svelte";
  import { sidebar } from "../stores/sidebar.svelte";
  import type { ArticleListItem } from "../types";
  import { viewTitle } from "../views";
  import FeedIcon from "./FeedIcon.svelte";
  import Icon from "./Icon.svelte";
  import VirtualList from "./VirtualList.svelte";

  /** Fixed row height keeps virtualised scrolling exact (SPEC §6.2). */
  const ROW_HEIGHT = 100;

  const title = $derived(
    articles.view.kind === "feed" && articles.view.id === sidebar.data?.deletedFeeds?.id
      ? t("sidebar.deletedFeeds")
      : viewTitle(articles.view, sidebar.data),
  );
  // The Starred view always lists every starred article, so the filter doesn't apply there.
  const filterable = $derived(articles.view.kind !== "starred" && articles.view.kind !== "unread");
  const unreadOnly = $derived(
    articles.view.kind === "unread" || (filterable && articles.unreadOnly),
  );
  const nextSort = $derived(articles.sort === "newestFirst" ? "oldestFirst" : "newestFirst");
  const selectedIndex = $derived(articles.items.findIndex((i) => i.id === articles.selectedId));

  let list = $state<ReturnType<typeof VirtualList<ArticleListItem>>>();

  // New view or filter: start at the top.
  $effect(() => {
    void articles.view;
    void articles.unreadOnly;
    void articles.sort;
    list?.scrollToTop();
  });
</script>

<section class="list-pane" aria-label={t("list.label")}>
  <header class="header">
    <h1 class="title">{title}</h1>
    <div class="controls">
      {#if filterable}
        <div class="segmented" role="group" aria-label={t("list.filter")}>
          <button aria-pressed={articles.unreadOnly} onclick={() => articles.setUnreadOnly(true)}
            >{t("list.filter.unread")}</button
          >
          <button aria-pressed={!articles.unreadOnly} onclick={() => articles.setUnreadOnly(false)}
            >{t("list.filter.all")}</button
          >
        </div>
      {/if}
      <button
        class="icon-button"
        title={t(`list.sort.${articles.sort}`)}
        aria-label={t(`list.sort.${nextSort}`)}
        onclick={() => articles.setSort(nextSort)}
      >
        <Icon name="sort" />
      </button>
    </div>
  </header>

  {#if articles.hasNewArticles}
    <button class="new-articles" onclick={() => articles.load()}>
      <Icon name="refresh" size={14} />
      {t("list.showNew")}
    </button>
  {/if}

  {#if articles.listError}
    <p class="message error" role="alert">
      {t("list.loadError", { message: articles.listError })}
    </p>
  {:else if sidebar.data && !sidebar.hasFeeds && !sidebar.data.deletedFeeds}
    <div class="welcome">
      <h2>{t("list.welcomeTitle")}</h2>
      <p>{t("list.welcomeText")}</p>
      <button class="button primary" onclick={() => dialogs.open({ kind: "addFeed" })}>
        <Icon name="plus" size={14} />
        {t("list.addFirstFeed")}
      </button>
    </div>
  {:else if articles.items.length === 0}
    <p class="message">
      {articles.loading ? t("list.loading") : unreadOnly ? t("list.emptyUnread") : t("list.empty")}
    </p>
  {:else}
    <VirtualList
      bind:this={list}
      items={articles.items}
      rowHeight={ROW_HEIGHT}
      key={(item) => item.id}
      label={t("list.label")}
      revealIndex={selectedIndex >= 0 ? selectedIndex : null}
      onEndReached={() => articles.loadMore()}
    >
      {#snippet row(item)}
        {@const feed = sidebar.feed(item.feedId)}
        <button
          class="row"
          class:unread={!item.isRead}
          aria-current={articles.selectedId === item.id ? "true" : undefined}
          onclick={() => articles.open(item.id)}
        >
          <span class="meta">
            <FeedIcon title={item.feedTitle} icon={feed?.icon ?? null} size={14} />
            <span class="feed">{item.feedTitle}</span>
            {#if item.publishedAt !== null}
              <span class="time">{relativeTime(item.publishedAt, clock.now)}</span>
            {/if}
          </span>
          <span class="headline" title={item.title}>
            {#if !item.isRead}
              <span class="dot" aria-hidden="true"></span>
              <span class="visually-hidden">{t("list.unread")}:</span>
            {/if}
            <span class="text">{item.title}</span>
            {#if item.isStarred}
              <span class="star" title={t("list.starred")}>
                <Icon name="star" size={13} filled />
                <span class="visually-hidden">{t("list.starred")}</span>
              </span>
            {/if}
          </span>
          <span class="summary">{item.summary}</span>
        </button>
      {/snippet}
    </VirtualList>
    {#if articles.loadingMore}
      <p class="loading-more">{t("list.loadingMore")}</p>
    {/if}
  {/if}
</section>

<style>
  .list-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    background: var(--bg-list);
    border-right: 1px solid var(--border);
  }

  .header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 10px 12px 10px 16px;
    border-bottom: 1px solid var(--border);
  }

  .title {
    flex: 1;
    min-width: 0;
    margin: 0;
    overflow: hidden;
    font-size: 1rem;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .segmented {
    display: inline-flex;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-input);
  }

  .segmented button {
    padding: 2px 8px;
    border: 0;
    border-radius: calc(var(--radius) - 2px);
    background: none;
    color: var(--text-muted);
    font-size: 0.8rem;
  }

  .segmented button[aria-pressed="true"] {
    background: var(--bg-selected);
    color: var(--accent-text);
    font-weight: 600;
  }

  .icon-button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 1px solid transparent;
    border-radius: var(--radius);
    background: none;
    color: var(--text-muted);
  }

  .icon-button:hover {
    border-color: var(--border);
    color: var(--text);
  }

  .new-articles {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 6px;
    margin: 8px 12px 0;
    padding: 5px 10px;
    border: 1px solid var(--accent);
    border-radius: 999px;
    background: var(--bg-selected);
    color: var(--accent-text);
    font-size: 0.82rem;
    font-weight: 600;
  }

  .message,
  .loading-more {
    margin: 32px 16px;
    color: var(--text-muted);
    text-align: center;
  }

  .loading-more {
    margin: 8px 16px;
    font-size: 0.8rem;
  }

  .message.error {
    color: var(--danger);
  }

  .welcome {
    margin: 20vh 24px 0;
    text-align: center;
  }

  .welcome h2 {
    margin: 0 0 6px;
    font-size: 1.1rem;
  }

  .welcome p {
    margin: 0 0 16px;
    color: var(--text-muted);
  }

  .row {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    height: 100%;
    padding: 10px 16px;
    border: 0;
    border-bottom: 1px solid var(--border-subtle);
    background: none;
    color: var(--text-muted);
    text-align: left;
  }

  .row:hover {
    background: var(--bg-hover);
  }

  .row[aria-current="true"] {
    background: var(--bg-selected);
  }

  .meta {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 0.75rem;
  }

  .feed {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .time {
    flex: none;
    font-variant-numeric: tabular-nums;
  }

  .headline {
    display: flex;
    align-items: center;
    gap: 6px;
    color: var(--text);
    font-size: 0.92rem;
    line-height: 1.35;
  }

  .unread .headline {
    font-weight: 650;
  }

  .row:not(.unread) .headline {
    color: var(--text-read);
  }

  .dot {
    flex: none;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: var(--accent);
  }

  .text {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .star {
    display: grid;
    flex: none;
    color: var(--star);
  }

  .summary {
    display: -webkit-box;
    overflow: hidden;
    font-size: 0.82rem;
    line-height: 1.4;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }
</style>
