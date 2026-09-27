<script lang="ts">
  import { t } from "../i18n";
  import { articles } from "../stores/articles.svelte";
  import { sidebar } from "../stores/sidebar.svelte";
  import type { FeedNode, View } from "../types";
  import { sameView } from "../views";
  import FeedIcon from "./FeedIcon.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import ThemeSwitcher from "./ThemeSwitcher.svelte";

  /** Feeds with this many consecutive errors get a warning (SPEC §6.1). */
  const ERROR_THRESHOLD = 3;

  const counts = $derived(sidebar.data?.counts);

  const smartViews = $derived<{ view: View; icon: IconName; label: string; count?: number }[]>([
    { view: { kind: "all" }, icon: "all", label: t("view.all") },
    { view: { kind: "unread" }, icon: "unread", label: t("view.unread"), count: counts?.unread },
    { view: { kind: "starred" }, icon: "star", label: t("view.starred"), count: counts?.starred },
    { view: { kind: "today" }, icon: "today", label: t("view.today"), count: counts?.today },
  ]);

  function isCurrent(view: View): "page" | undefined {
    return sameView(view, articles.view) ? "page" : undefined;
  }
</script>

{#snippet count(n: number | undefined, label: string)}
  {#if n}
    <span class="count" aria-label={label}>{n.toLocaleString()}</span>
  {/if}
{/snippet}

{#snippet feedItem(feed: FeedNode)}
  {@const view: View = { kind: "feed", id: feed.id }}
  <li>
    <button
      class="item feed"
      aria-current={isCurrent(view)}
      onclick={() => articles.showView(view)}
    >
      <FeedIcon title={feed.title} />
      <span class="name">{feed.title}</span>
      {#if feed.errorCount >= ERROR_THRESHOLD}
        <span class="warning" title={t("sidebar.feedErrors", { count: feed.errorCount })}>
          <Icon name="warning" size={14} />
          <span class="visually-hidden">{t("sidebar.feedErrors", { count: feed.errorCount })}</span>
        </span>
      {/if}
      {@render count(feed.unreadCount, t("sidebar.unreadCount", { count: feed.unreadCount }))}
    </button>
  </li>
{/snippet}

<nav class="sidebar" aria-label={t("sidebar.label")}>
  <header class="brand">
    <img src="/logo.svg" alt="" width="22" height="22" />
    <span>omarss</span>
  </header>

  <div class="scroll">
    <h2 class="visually-hidden">{t("sidebar.views")}</h2>
    <ul class="list">
      {#each smartViews as item (item.view.kind)}
        <li>
          <button
            class="item"
            aria-current={isCurrent(item.view)}
            onclick={() => articles.showView(item.view)}
          >
            <Icon name={item.icon} />
            <span class="name">{item.label}</span>
            {@render count(
              item.count,
              item.view.kind === "starred"
                ? t("sidebar.starredCount", { count: item.count ?? 0 })
                : t("sidebar.unreadCount", { count: item.count ?? 0 }),
            )}
          </button>
        </li>
      {/each}
    </ul>

    <h2 class="section">{t("sidebar.feeds")}</h2>
    <ul class="list">
      {#each sidebar.data?.folders ?? [] as folder (folder.id)}
        {@const view: View = { kind: "folder", id: folder.id }}
        {@const collapsed = sidebar.isCollapsed(folder.id)}
        <li>
          <div class="folder-row">
            <button
              class="toggle"
              aria-expanded={!collapsed}
              aria-label={collapsed
                ? t("sidebar.expandFolder", { name: folder.name })
                : t("sidebar.collapseFolder", { name: folder.name })}
              onclick={() => sidebar.toggleFolder(folder.id)}
            >
              <span class="chevron" class:open={!collapsed}><Icon name="chevron" size={14} /></span>
            </button>
            <button
              class="item folder"
              aria-current={isCurrent(view)}
              onclick={() => articles.showView(view)}
            >
              <Icon name="folder" />
              <span class="name">{folder.name}</span>
              {@render count(
                folder.unreadCount,
                t("sidebar.unreadCount", { count: folder.unreadCount }),
              )}
            </button>
          </div>
          {#if !collapsed}
            <ul class="list nested">
              {#each folder.feeds as feed (feed.id)}
                {@render feedItem(feed)}
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
      {#each sidebar.data?.feeds ?? [] as feed (feed.id)}
        {@render feedItem(feed)}
      {/each}
    </ul>
  </div>

  <footer class="footer">
    <span class="demo">{t("sidebar.demoData")}</span>
    <ThemeSwitcher />
  </footer>
</nav>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    min-width: 0;
    background: var(--bg-sidebar);
    border-right: 1px solid var(--border);
  }

  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 16px 10px;
    font-weight: 700;
    font-size: 1.05rem;
    letter-spacing: 0.01em;
  }

  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 4px 8px 12px;
  }

  .section {
    margin: 18px 8px 6px;
    color: var(--text-muted);
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nested {
    padding-left: 18px;
  }

  .folder-row {
    display: flex;
    align-items: center;
  }

  .toggle {
    display: grid;
    place-items: center;
    width: 18px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--text-muted);
  }

  .chevron {
    display: grid;
    transition: transform 120ms ease;
  }

  .chevron.open {
    transform: rotate(90deg);
  }

  .item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    min-width: 0;
    height: 30px;
    padding: 0 8px;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--text);
    text-align: left;
  }

  .folder-row .item {
    padding-left: 4px;
  }

  .item:hover {
    background: var(--bg-hover);
  }

  .item[aria-current="page"] {
    background: var(--bg-selected);
    color: var(--accent-text);
    font-weight: 600;
  }

  .name {
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .count {
    color: var(--text-muted);
    font-size: 0.78rem;
    font-variant-numeric: tabular-nums;
  }

  .warning {
    display: grid;
    color: var(--warning);
  }

  .footer {
    display: flex;
    align-items: center;
    justify-content: space-between;
    flex-wrap: wrap;
    gap: 8px;
    padding: 10px 12px;
    border-top: 1px solid var(--border);
  }

  .demo {
    color: var(--text-muted);
    font-size: 0.75rem;
  }
</style>
