<script lang="ts">
  import { relativeTime } from "../format";
  import { t } from "../i18n";
  import { articles } from "../stores/articles.svelte";
  import { clock } from "../stores/clock.svelte";
  import { sidebar } from "../stores/sidebar.svelte";
  import { viewTitle } from "../views";
  import FeedIcon from "./FeedIcon.svelte";
  import Icon from "./Icon.svelte";

  const title = $derived(viewTitle(articles.view, sidebar.data));
  // The Starred view always lists every starred article, so the filter doesn't apply there.
  const filterable = $derived(articles.view.kind !== "starred" && articles.view.kind !== "unread");
  const unreadOnly = $derived(
    articles.view.kind === "unread" || (filterable && articles.unreadOnly),
  );
  const nextSort = $derived(articles.sort === "newestFirst" ? "oldestFirst" : "newestFirst");
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

  {#if articles.listError}
    <p class="message error" role="alert">
      {t("list.loadError", { message: articles.listError })}
    </p>
  {:else if articles.items.length === 0}
    <p class="message">
      {unreadOnly ? t("list.emptyUnread") : t("list.empty")}
    </p>
  {:else}
    <ul class="items">
      {#each articles.items as item (item.id)}
        <li>
          <button
            class="row"
            class:unread={!item.isRead}
            aria-current={articles.selectedId === item.id ? "true" : undefined}
            onclick={() => articles.open(item.id)}
          >
            <span class="meta">
              <FeedIcon title={item.feedTitle} size={14} />
              <span class="feed">{item.feedTitle}</span>
              {#if item.publishedAt !== null}
                <span class="time">{relativeTime(item.publishedAt, clock.now)}</span>
              {/if}
            </span>
            <span class="headline">
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
        </li>
      {/each}
    </ul>
  {/if}
</section>

<style>
  .list-pane {
    display: flex;
    flex-direction: column;
    min-width: 0;
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

  .message {
    margin: 32px 16px;
    color: var(--text-muted);
    text-align: center;
  }

  .message.error {
    color: var(--danger);
  }

  .items {
    flex: 1;
    margin: 0;
    padding: 0;
    overflow-y: auto;
    list-style: none;
  }

  .row {
    display: flex;
    flex-direction: column;
    gap: 3px;
    width: 100%;
    padding: 10px 16px 11px;
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
    align-items: baseline;
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
    transform: translateY(-1px);
  }

  .text {
    flex: 1;
  }

  .star {
    display: grid;
    flex: none;
    align-self: center;
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
