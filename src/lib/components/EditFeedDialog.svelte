<script lang="ts">
  import { api, errorMessage } from "../api";
  import { relativeTime } from "../format";
  import { t } from "../i18n";
  import { articles } from "../stores/app.svelte";
  import { clock } from "../stores/clock.svelte";
  import { dialogs } from "../stores/dialogs.svelte";
  import { sidebar } from "../stores/sidebar.svelte";
  import type { FeedDetails } from "../types";
  import Dialog from "./Dialog.svelte";

  let {
    feedId,
    focusUrl = false,
    onclose,
  }: { feedId: number; focusUrl?: boolean; onclose: () => void } = $props();

  const INTERVALS = [900, 1800, 3600, 7200, 21600, 0] as const;

  let details = $state<FeedDetails | null>(null);
  let url = $state("");
  let userAgent = $state("");
  let urlInput = $state<HTMLInputElement>();
  let name = $state("");
  let folderId = $state<number | null>(null);
  let interval = $state<number | null>(null);
  let paused = $state(false);
  let error = $state<string | null>(null);
  let saving = $state(false);

  $effect(() => {
    api
      .getFeed(feedId)
      .then((feed) => {
        details = feed;
        url = feed.url;
        userAgent = feed.userAgent ?? "";
        name = feed.customTitle ?? feed.title;
        folderId = feed.folderId;
        interval = feed.fetchInterval;
        paused = feed.paused;
      })
      .catch((e) => (error = errorMessage(e)));
  });

  async function save() {
    if (!details) return;
    saving = true;
    error = null;
    try {
      await api.updateFeed(feedId, {
        url: url.trim(),
        customTitle: name.trim() && name.trim() !== details.title ? name.trim() : null,
        folderId,
        fetchInterval: interval,
        paused,
        userAgent: userAgent.trim() || null,
      });
      onclose();
      await sidebar.load();
      await articles.load();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      saving = false;
    }
  }

  // From "Broken feeds → Edit URL": start in the address field.
  $effect(() => {
    if (focusUrl && urlInput) {
      urlInput.focus();
      urlInput.select();
    }
  });

  function unsubscribe() {
    if (!details) return;
    const feed = details;
    dialogs.open({
      kind: "confirm",
      title: t("confirm.unsubscribe.title", { name: feed.customTitle ?? feed.title }),
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
</script>

<Dialog title={t("editFeed.title")} {onclose}>
  {#if error}
    <p class="form-error" role="alert">{error}</p>
  {/if}
  {#if details}
    <form
      id="edit-feed"
      onsubmit={(e) => {
        e.preventDefault();
        save();
      }}
    >
      <label class="field">
        <span>{t("editFeed.name")}</span>
        <input type="text" bind:value={name} placeholder={details.title} />
      </label>
      <label class="field">
        <span>{t("editFeed.address")}</span>
        <input type="url" bind:value={url} bind:this={urlInput} spellcheck="false" required />
        {#if url.trim() !== details.url}
          <span class="hint">{t("editFeed.addressChanged")}</span>
        {/if}
      </label>
      <label class="field">
        <span>{t("editFeed.folder")}</span>
        <select bind:value={folderId}>
          <option value={null}>{t("addFeed.noFolder")}</option>
          {#each sidebar.data?.folders ?? [] as folder (folder.id)}
            <option value={folder.id}>{folder.name}</option>
          {/each}
        </select>
      </label>
      <label class="field">
        <span>{t("editFeed.interval")}</span>
        <select bind:value={interval}>
          <option value={null}>{t("editFeed.interval.default")}</option>
          {#each INTERVALS as seconds (seconds)}
            <option value={seconds}>{t(`editFeed.interval.${seconds}`)}</option>
          {/each}
        </select>
      </label>
      <label class="check">
        <input type="checkbox" bind:checked={paused} />
        {t("editFeed.paused")}
      </label>
      <details class="advanced" open={!!details.userAgent}>
        <summary>{t("editFeed.advanced")}</summary>
        <label class="field">
          <span>{t("editFeed.userAgent")}</span>
          <input
            type="text"
            bind:value={userAgent}
            placeholder={t("editFeed.userAgent.placeholder")}
            spellcheck="false"
          />
          <span class="hint">{t("editFeed.userAgent.hint")}</span>
        </label>
      </details>
      <p class="status">
        {details.lastFetchedAt !== null
          ? t("editFeed.lastFetched", { time: relativeTime(details.lastFetchedAt, clock.now) })
          : t("editFeed.neverFetched")}
      </p>
      {#if details.lastError}
        <p class="status error">{t("editFeed.lastError", { message: details.lastError })}</p>
      {/if}
    </form>
  {/if}

  {#snippet footer()}
    <button class="button danger-link" onclick={unsubscribe} disabled={!details}>
      {t("editFeed.unsubscribe")}
    </button>
    <span class="spacer"></span>
    <button class="button" onclick={onclose}>{t("dialog.cancel")}</button>
    <button class="button primary" type="submit" form="edit-feed" disabled={!details || saving}>
      {t("dialog.save")}
    </button>
  {/snippet}
</Dialog>

<style>
  .check {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
  }

  .hint {
    color: var(--text-muted);
    font-size: 0.8rem;
    font-weight: normal;
  }

  .advanced {
    margin-bottom: 12px;
  }

  .advanced summary {
    margin-bottom: 10px;
    color: var(--text-muted);
    font-size: 0.85rem;
    cursor: pointer;
  }

  .status {
    margin: 0 0 4px;
    color: var(--text-muted);
    font-size: 0.82rem;
  }

  .status.error {
    color: var(--danger);
  }

  .spacer {
    flex: 1;
  }

  .danger-link {
    border-color: transparent;
    background: none;
    color: var(--danger);
  }
</style>
