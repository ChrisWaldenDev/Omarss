<script lang="ts">
  import { exportOpml, importOpml } from "../actions";
  import { api, errorMessage } from "../api";
  import { relativeTime } from "../format";
  import { t } from "../i18n";
  import { currentOrder, moveFeed, moveFolder, type FeedDrop } from "../sidebarOrder";
  import { articles } from "../stores/app.svelte";
  import { clock } from "../stores/clock.svelte";
  import { dialogs } from "../stores/dialogs.svelte";
  import { refresh } from "../stores/refresh.svelte";
  import { sidebar } from "../stores/sidebar.svelte";
  import { toasts } from "../stores/toasts.svelte";
  import type { FeedNode, FolderNode, Sidebar, View } from "../types";
  import { brokenFeeds, isBroken, sameView } from "../views";
  import ContextMenu, { type MenuItem } from "./ContextMenu.svelte";
  import FeedIcon from "./FeedIcon.svelte";
  import Icon, { type IconName } from "./Icon.svelte";
  import ThemeSwitcher from "./ThemeSwitcher.svelte";

  const counts = $derived(sidebar.data?.counts);
  const smartViews = $derived<{ view: View; icon: IconName; label: string; count?: number }[]>([
    { view: { kind: "all" }, icon: "all", label: t("view.all") },
    { view: { kind: "unread" }, icon: "unread", label: t("view.unread"), count: counts?.unread },
    { view: { kind: "starred" }, icon: "star", label: t("view.starred"), count: counts?.starred },
    { view: { kind: "today" }, icon: "today", label: t("view.today"), count: counts?.today },
  ]);

  const status = $derived(
    refresh.running && refresh.total > 0
      ? t("sidebar.refreshing", { done: refresh.done, total: refresh.total })
      : refresh.offline
        ? t("sidebar.offline")
        : refresh.lastFinishedAt !== null
          ? t("sidebar.updatedAgo", { time: relativeTime(refresh.lastFinishedAt, clock.now) })
          : "",
  );

  const broken = $derived(brokenFeeds(sidebar.data).length);

  function isCurrent(view: View): "page" | undefined {
    return !articles.brokenFeedsOpen && sameView(view, articles.view) ? "page" : undefined;
  }

  function appMenu(event: MouseEvent) {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    menu = {
      x: box.left,
      y: box.bottom + 4,
      items: [
        { label: t("menu.importOpml"), action: importOpml },
        { label: t("menu.exportOpml"), action: exportOpml },
        { label: t("menu.shortcuts"), action: () => dialogs.open({ kind: "shortcuts" }) },
        { label: t("menu.settings"), action: () => dialogs.open({ kind: "settings" }) },
      ],
    };
  }

  // ---- Context menus ----------------------------------------------------------------------
  let menu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  function openMenu(event: MouseEvent, items: MenuItem[]) {
    event.preventDefault();
    menu = { x: event.clientX, y: event.clientY, items };
  }

  function feedMenu(feed: FeedNode): MenuItem[] {
    const items: MenuItem[] = [
      { label: t("menu.refresh"), action: () => refresh.request({ kind: "feed", id: feed.id }) },
      { label: t("menu.edit"), action: () => dialogs.open({ kind: "editFeed", feedId: feed.id }) },
    ];
    if (feed.siteUrl) {
      const site = feed.siteUrl;
      items.push({ label: t("menu.openSite"), action: () => api.openExternal(site) });
    }
    items.push({
      label: t("menu.unsubscribe"),
      danger: true,
      action: () => confirmUnsubscribe(feed),
    });
    return items;
  }

  function folderMenu(folder: FolderNode): MenuItem[] {
    return [
      {
        label: t("menu.refresh"),
        action: () => refresh.request({ kind: "folder", id: folder.id }),
      },
      {
        label: t("menu.rename"),
        action: () => dialogs.open({ kind: "folder", folderId: folder.id, name: folder.name }),
      },
      {
        label: t("menu.deleteFolder"),
        danger: true,
        action: () =>
          dialogs.open({
            kind: "confirm",
            title: t("confirm.deleteFolder.title", { name: folder.name }),
            message: t("confirm.deleteFolder.message"),
            confirmLabel: t("confirm.deleteFolder.confirm"),
            onConfirm: async () => {
              await api.deleteFolder(folder.id);
              await sidebar.load();
              if (articles.view.kind === "folder" && articles.view.id === folder.id) {
                await articles.showView({ kind: "all" });
              }
            },
          }),
      },
    ];
  }

  function confirmUnsubscribe(feed: FeedNode) {
    dialogs.open({
      kind: "confirm",
      title: t("confirm.unsubscribe.title", { name: feed.title }),
      message: t("confirm.unsubscribe.message"),
      confirmLabel: t("confirm.unsubscribe.confirm"),
      onConfirm: async () => {
        await api.unsubscribeFeed(feed.id);
        if (articles.article?.feedId === feed.id) articles.close();
        await sidebar.load();
        if (articles.view.kind === "feed" && articles.view.id === feed.id) {
          await articles.showView({ kind: "all" });
        } else {
          await articles.load();
        }
      },
    });
  }

  // ---- Drag and drop (SPEC §6.1) -----------------------------------------------------------
  let dragging = $state<{ kind: "feed" | "folder"; id: number } | null>(null);
  let dropHint = $state<{ key: string; where: "before" | "after" | "into" } | null>(null);

  function startDrag(event: DragEvent, kind: "feed" | "folder", id: number) {
    dragging = { kind, id };
    event.dataTransfer?.setData("text/plain", `${kind}:${id}`);
    if (event.dataTransfer) event.dataTransfer.effectAllowed = "move";
  }

  function endDrag() {
    dragging = null;
    dropHint = null;
  }

  function half(event: DragEvent): "before" | "after" {
    const box = (event.currentTarget as HTMLElement).getBoundingClientRect();
    return event.clientY < box.top + box.height / 2 ? "before" : "after";
  }

  function overFeed(event: DragEvent, feed: FeedNode) {
    if (dragging?.kind !== "feed" || dragging.id === feed.id) return;
    event.preventDefault();
    dropHint = { key: `feed-${feed.id}`, where: half(event) };
  }

  function overFolder(event: DragEvent, folder: FolderNode) {
    if (!dragging) return;
    event.preventDefault();
    dropHint =
      dragging.kind === "feed"
        ? { key: `folder-${folder.id}`, where: "into" }
        : dragging.id === folder.id
          ? null
          : { key: `folder-${folder.id}`, where: half(event) };
  }

  function overRoot(event: DragEvent) {
    if (dragging?.kind !== "feed") return;
    event.preventDefault();
    dropHint = { key: "root", where: "into" };
  }

  async function drop(event: DragEvent, target: FeedDrop | { kind: "folder"; folderId: number }) {
    event.preventDefault();
    const moving = dragging;
    const hint = dropHint;
    endDrag();
    const current = sidebar.data;
    if (!moving || !current) return;
    let next: Sidebar = current;
    if (moving.kind === "feed") {
      const feedDrop: FeedDrop =
        target.kind === "folder"
          ? { kind: "intoFolder", folderId: target.folderId }
          : target.kind === "beforeFeed" || target.kind === "afterFeed"
            ? { kind: hint?.where === "before" ? "beforeFeed" : "afterFeed", feedId: target.feedId }
            : target;
      next = moveFeed(current, moving.id, feedDrop);
    } else if (target.kind === "folder") {
      next = moveFolder(current, moving.id, {
        kind: hint?.where === "before" ? "beforeFolder" : "afterFolder",
        folderId: target.folderId,
      });
    }
    if (next === current) return;
    sidebar.data = next;
    try {
      await api.reorderSidebar(currentOrder(next));
    } catch (error) {
      sidebar.data = current;
      toasts.show(t("error.action", { message: errorMessage(error) }));
    }
  }

  function hintClass(key: string): string {
    return dropHint?.key === key ? `drop-${dropHint.where}` : "";
  }
</script>

{#snippet count(n: number | undefined, label: string)}
  {#if n}
    <span class="count" aria-label={label}>{n.toLocaleString()}</span>
  {/if}
{/snippet}

{#snippet feedRow(feed: FeedNode, deleted = false)}
  {@const view: View = { kind: "feed", id: feed.id }}
  <li
    class={hintClass(`feed-${feed.id}`)}
    ondragover={deleted ? undefined : (e) => overFeed(e, feed)}
    ondrop={deleted ? undefined : (e) => drop(e, { kind: "afterFeed", feedId: feed.id })}
  >
    <button
      class="item feed"
      class:paused={feed.paused && !deleted}
      aria-current={isCurrent(view)}
      draggable={!deleted}
      ondragstart={(e) => startDrag(e, "feed", feed.id)}
      ondragend={endDrag}
      oncontextmenu={deleted ? undefined : (e) => openMenu(e, feedMenu(feed))}
      onclick={() => articles.showView(view)}
      title={feed.paused && !deleted ? t("sidebar.paused") : undefined}
    >
      {#if deleted}
        <Icon name="trash" />
      {:else}
        <FeedIcon title={feed.title} icon={feed.icon} />
      {/if}
      <span class="name">{deleted ? t("sidebar.deletedFeeds") : feed.title}</span>
      {#if !deleted && isBroken(feed)}
        <span
          class="warning"
          title={feed.lastError ?? t("sidebar.feedErrors", { count: feed.errorCount })}
        >
          <Icon name="warning" size={14} />
          <span class="visually-hidden">{t("sidebar.feedErrors", { count: feed.errorCount })}</span>
        </span>
      {:else if feed.paused && !deleted}
        <span class="muted-icon"><Icon name="pause" size={13} /></span>
      {/if}
      {@render count(feed.unreadCount, t("sidebar.unreadCount", { count: feed.unreadCount }))}
    </button>
  </li>
{/snippet}

<nav class="sidebar" aria-label={t("sidebar.label")}>
  <header class="brand">
    <img src="/logo.svg" alt="" width="22" height="22" />
    <span class="app-name">Omarss</span>
    <button
      class="icon-button"
      title={t("sidebar.addFeed")}
      aria-label={t("sidebar.addFeed")}
      onclick={() => dialogs.open({ kind: "addFeed" })}
    >
      <Icon name="plus" />
    </button>
    <button
      class="icon-button"
      title={t("sidebar.newFolder")}
      aria-label={t("sidebar.newFolder")}
      onclick={() => dialogs.open({ kind: "folder", folderId: null, name: "" })}
    >
      <Icon name="folderPlus" />
    </button>
    <button
      class="icon-button"
      class:spinning={refresh.running}
      title={t("sidebar.refreshAll")}
      aria-label={t("sidebar.refreshAll")}
      onclick={() => refresh.request({ kind: "all" })}
    >
      <Icon name="refresh" />
    </button>
    <button
      class="icon-button"
      title={t("sidebar.more")}
      aria-label={t("sidebar.more")}
      aria-haspopup="menu"
      onclick={appMenu}
    >
      <Icon name="more" />
    </button>
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
      {#if broken > 0}
        <li>
          <button
            class="item broken"
            aria-current={articles.brokenFeedsOpen ? "page" : undefined}
            onclick={() => articles.showBrokenFeeds()}
          >
            <Icon name="warning" />
            <span class="name">{t("broken.title")}</span>
            {@render count(broken, t("broken.count", { count: broken }))}
          </button>
        </li>
      {/if}
    </ul>

    <h2
      class="section {hintClass('root')}"
      ondragover={overRoot}
      ondrop={(e) => drop(e, { kind: "root" })}
    >
      {t("sidebar.feeds")}
    </h2>
    {#if !sidebar.hasFeeds}
      <p class="empty">{t("sidebar.noFeeds")}</p>
    {/if}
    <ul class="list">
      {#each sidebar.data?.folders ?? [] as folder (folder.id)}
        {@const view: View = { kind: "folder", id: folder.id }}
        {@const collapsed = sidebar.isCollapsed(folder.id)}
        <li>
          <div
            class="folder-row {hintClass(`folder-${folder.id}`)}"
            role="presentation"
            ondragover={(e) => overFolder(e, folder)}
            ondrop={(e) => drop(e, { kind: "folder", folderId: folder.id })}
          >
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
              draggable="true"
              ondragstart={(e) => startDrag(e, "folder", folder.id)}
              ondragend={endDrag}
              oncontextmenu={(e) => openMenu(e, folderMenu(folder))}
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
                {@render feedRow(feed)}
              {/each}
            </ul>
          {/if}
        </li>
      {/each}
      {#each sidebar.data?.feeds ?? [] as feed (feed.id)}
        {@render feedRow(feed)}
      {/each}
    </ul>
    {#if dragging?.kind === "feed"}
      <div
        class="root-drop {hintClass('root')}"
        role="presentation"
        ondragover={overRoot}
        ondrop={(e) => drop(e, { kind: "root" })}
      >
        {t("sidebar.dropToTop")}
      </div>
    {/if}
    {#if sidebar.data?.deletedFeeds}
      <ul class="list deleted">
        {@render feedRow(sidebar.data.deletedFeeds, true)}
      </ul>
    {/if}
  </div>

  <footer class="footer">
    <span class="status" class:offline={refresh.offline && !refresh.running} role="status">
      {#if refresh.offline && !refresh.running}<Icon name="offline" size={13} />{/if}
      {status}
    </span>
    <div class="footer-row">
      <ThemeSwitcher />
      <button
        class="icon-button"
        title={t("menu.settings")}
        aria-label={t("menu.settings")}
        onclick={() => dialogs.open({ kind: "settings" })}
      >
        <Icon name="settings" />
      </button>
    </div>
  </footer>
</nav>

{#if menu}
  <ContextMenu x={menu.x} y={menu.y} items={menu.items} onclose={() => (menu = null)} />
{/if}

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
    gap: 6px;
    padding: 12px 10px 8px 16px;
  }

  .app-name {
    flex: 1;
    margin-left: 2px;
    font-size: 1.05rem;
    font-weight: 700;
    letter-spacing: 0.01em;
  }

  .icon-button {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--text-muted);
  }

  .icon-button:hover {
    background: var(--bg-hover);
    color: var(--text);
  }

  .spinning :global(svg) {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    to {
      transform: rotate(360deg);
    }
  }

  .scroll {
    flex: 1;
    overflow-y: auto;
    padding: 4px 8px 12px;
  }

  .section {
    margin: 18px 0 6px;
    padding: 2px 8px;
    border-radius: var(--radius);
    color: var(--text-muted);
    font-size: 0.72rem;
    font-weight: 600;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  .empty {
    margin: 4px 8px;
    color: var(--text-muted);
    font-size: 0.85rem;
  }

  .list {
    margin: 0;
    padding: 0;
    list-style: none;
  }

  .nested {
    padding-left: 18px;
  }

  .deleted {
    margin-top: 12px;
    padding-top: 8px;
    border-top: 1px solid var(--border);
  }

  .folder-row {
    display: flex;
    align-items: center;
    border-radius: var(--radius);
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

  .item.paused .name {
    color: var(--text-muted);
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

  .muted-icon {
    display: grid;
    color: var(--text-muted);
  }

  /* Drop indicators */
  .drop-before {
    box-shadow: inset 0 2px 0 var(--accent);
  }

  .drop-after {
    box-shadow: inset 0 -2px 0 var(--accent);
  }

  .drop-into {
    background: var(--bg-selected);
    outline: 1px dashed var(--accent);
  }

  .root-drop {
    margin-top: 6px;
    padding: 8px;
    border: 1px dashed var(--border);
    border-radius: var(--radius);
    color: var(--text-muted);
    font-size: 0.8rem;
    text-align: center;
  }

  .footer {
    display: flex;
    flex-direction: column;
    align-items: flex-start;
    gap: 8px;
    padding: 10px 12px;
    border-top: 1px solid var(--border);
  }

  .footer-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    align-self: stretch;
  }

  .item.broken :global(svg) {
    color: var(--warning);
  }

  .status {
    display: flex;
    align-items: center;
    gap: 6px;
    min-height: 1.2em;
    color: var(--text-muted);
    font-size: 0.75rem;
  }

  .status.offline {
    color: var(--warning);
  }
</style>
