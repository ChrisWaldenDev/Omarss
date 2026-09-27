<script lang="ts">
  import { onMount } from "svelte";

  import { errorMessage } from "./lib/api";
  import ArticleList from "./lib/components/ArticleList.svelte";
  import Dialogs from "./lib/components/Dialogs.svelte";
  import ReaderPane from "./lib/components/ReaderPane.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import Toasts from "./lib/components/Toasts.svelte";
  import { t } from "./lib/i18n";
  import { articles, connectEvents } from "./lib/stores/app.svelte";
  import { clock } from "./lib/stores/clock.svelte";
  import { refresh } from "./lib/stores/refresh.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { sidebar } from "./lib/stores/sidebar.svelte";
  import { applyTheme } from "./lib/theme";

  let ready = $state(false);
  let startError = $state<string | null>(null);

  onMount(() => {
    let disconnect: (() => void) | undefined;
    let cancelled = false;
    connectEvents()
      .then((stop) => (cancelled ? stop() : (disconnect = stop)))
      .catch((error) => console.error("could not listen for events", error));
    Promise.all([settings.load(), sidebar.load(), articles.load(), refresh.load()])
      .catch((error) => (startError = errorMessage(error)))
      .finally(() => (ready = true));
    const stopClock = clock.start();
    return () => {
      cancelled = true;
      disconnect?.();
      stopClock();
    };
  });

  $effect(() => {
    if (settings.current) applyTheme(settings.current.theme, document.documentElement);
  });
</script>

{#if startError}
  <p class="start-error" role="alert">{t("app.startError", { message: startError })}</p>
{:else if ready}
  <div class="layout">
    <Sidebar />
    <ArticleList />
    <ReaderPane />
  </div>
  <Dialogs />
  <Toasts />
{/if}

<style>
  .layout {
    display: grid;
    grid-template-columns:
      clamp(200px, 18vw, 280px)
      clamp(260px, 28vw, 420px)
      minmax(0, 1fr);
    /* One row exactly the window's height; each pane scrolls on its own. */
    grid-template-rows: minmax(0, 1fr);
    height: 100vh;
  }

  .start-error {
    margin: 30vh auto 0;
    max-width: 40rem;
    padding: 0 24px;
    color: var(--danger);
    text-align: center;
  }
</style>
