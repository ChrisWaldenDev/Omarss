<script lang="ts">
  import { onMount } from "svelte";

  import { attempt, runAction } from "./lib/actions";
  import { api, errorMessage } from "./lib/api";
  import ArticleList from "./lib/components/ArticleList.svelte";
  import BrokenFeeds from "./lib/components/BrokenFeeds.svelte";
  import Dialogs from "./lib/components/Dialogs.svelte";
  import ReaderPane from "./lib/components/ReaderPane.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Splitter from "./lib/components/Splitter.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import { t } from "./lib/i18n";
  import { eventToKey, isTypingTarget, KeyDispatcher, resolveBindings } from "./lib/keyboard";
  import { listenThenLoad } from "./lib/startup";
  import { articles, connectEvents } from "./lib/stores/app.svelte";
  import { clock } from "./lib/stores/clock.svelte";
  import { dialogs } from "./lib/stores/dialogs.svelte";
  import { refresh } from "./lib/stores/refresh.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { sidebar } from "./lib/stores/sidebar.svelte";
  import { applyAppearance, applyCustomCss, applyTheme } from "./lib/theme";

  let ready = $state(false);
  let startError = $state<string | null>(null);

  onMount(() => {
    const disconnect = listenThenLoad(connectEvents, () =>
      Promise.all([settings.load(), sidebar.load(), articles.load(), refresh.load()])
        .then(() => {})
        .catch((error) => {
          startError = errorMessage(error);
        })
        .finally(() => (ready = true)),
    );
    const stopClock = clock.start();
    api
      .getCustomCss()
      .then((css) => applyCustomCss(css, document))
      .catch((error) => console.error("custom.css failed to load", error));
    return () => {
      disconnect();
      stopClock();
    };
  });

  $effect(() => {
    const current = settings.current;
    if (!current) return;
    applyTheme(current.theme, document.documentElement);
    applyAppearance(current, document.documentElement.style);
    articles.markRead = {
      mode: current.markReadMode,
      delaySeconds: current.markReadDelaySeconds,
    };
  });

  // ---- Layout (SPEC §6.2, §6.8) --------------------------------------------------------------
  const twoPane = $derived(settings.current?.layout === "twoPane");
  // Widths follow the settings, and the splitters override them while dragging.
  let sidebarWidth = $derived(settings.current?.sidebarWidth ?? 240);
  let listWidth = $derived(settings.current?.listWidth ?? 360);
  const readerOpen = $derived(articles.article !== null || articles.articleError !== null);
  const columns = $derived(
    twoPane
      ? `min(${sidebarWidth}px, 40vw) 0 minmax(0, 1fr)`
      : `min(${sidebarWidth}px, 30vw) 0 min(${listWidth}px, 40vw) 0 minmax(0, 1fr)`,
  );

  // ---- Keyboard shortcuts (SPEC §6.7) --------------------------------------------------------
  const bindings = $derived(resolveBindings(settings.current?.shortcuts ?? {}));
  const dispatcher = new KeyDispatcher(() => bindings);

  function onkeydown(event: KeyboardEvent) {
    if (event.defaultPrevented || event.isComposing || dialogs.current) return;
    const target = event.target as HTMLElement | null;
    if (isTypingTarget(target) || target?.closest?.("[role='menu']")) return;
    if (event.key === "Escape") {
      if (twoPane && readerOpen) {
        event.preventDefault();
        articles.close();
      }
      dispatcher.reset();
      return;
    }
    const key = eventToKey(event);
    if (!key) return;
    // Enter and Space keep their usual meaning on focused buttons, links and controls, except
    // on article rows, where Space scrolls the reader.
    const control = target?.closest?.("button, a[href], summary, [role='separator']");
    if (
      (key === "Enter" || key === "Space") &&
      control &&
      !control.hasAttribute("data-article-row")
    ) {
      return;
    }
    const action = dispatcher.handle(key);
    if (action === null) return;
    event.preventDefault();
    if (action !== "pending") void runAction(action);
  }

  function saveWidths() {
    void attempt(() => settings.update({ sidebarWidth, listWidth }));
  }
</script>

<svelte:window {onkeydown} />

{#if startError}
  <p class="start-error" role="alert">{t("app.startError", { message: startError })}</p>
{:else if ready}
  <div class="layout" class:two-pane={twoPane} style:grid-template-columns={columns}>
    <Sidebar />
    <Splitter
      value={sidebarWidth}
      min={180}
      max={480}
      label={t("layout.resizeSidebar")}
      onchange={(width) => (sidebarWidth = width)}
      oncommit={saveWidths}
    />
    {#if articles.brokenFeedsOpen}
      <div class="span-rest"><BrokenFeeds /></div>
    {:else if twoPane}
      {#if readerOpen}
        <ReaderPane onback={() => articles.close()} />
      {:else}
        <ArticleList />
      {/if}
    {:else}
      <ArticleList />
      <Splitter
        value={listWidth}
        min={240}
        max={720}
        label={t("layout.resizeList")}
        onchange={(width) => (listWidth = width)}
        oncommit={saveWidths}
      />
      <ReaderPane />
    {/if}
  </div>
  <Dialogs />
  <Toasts />
{/if}

<style>
  .layout {
    display: grid;
    /* One row exactly the window's height; each pane scrolls on its own. */
    grid-template-rows: minmax(0, 1fr);
    height: 100vh;
  }

  .span-rest {
    display: grid;
    grid-column: 3 / -1;
    min-width: 0;
    min-height: 0;
  }

  .start-error {
    margin: 30vh auto 0;
    max-width: 40rem;
    padding: 0 24px;
    color: var(--danger);
    text-align: center;
  }
</style>
