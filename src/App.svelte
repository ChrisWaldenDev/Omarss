<script lang="ts">
  import { onMount } from "svelte";

  import { errorMessage } from "./lib/api";
  import ArticleList from "./lib/components/ArticleList.svelte";
  import ReaderPane from "./lib/components/ReaderPane.svelte";
  import Sidebar from "./lib/components/Sidebar.svelte";
  import { t } from "./lib/i18n";
  import { articles } from "./lib/stores/articles.svelte";
  import { clock } from "./lib/stores/clock.svelte";
  import { settings } from "./lib/stores/settings.svelte";
  import { sidebar } from "./lib/stores/sidebar.svelte";
  import { applyTheme } from "./lib/theme";

  let ready = $state(false);
  let startError = $state<string | null>(null);

  onMount(() => {
    Promise.all([settings.load(), sidebar.load(), articles.load()])
      .catch((error) => (startError = errorMessage(error)))
      .finally(() => (ready = true));
    return clock.start();
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
{/if}

<style>
  .layout {
    display: grid;
    grid-template-columns:
      clamp(180px, 18vw, 280px)
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
