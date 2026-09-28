<script lang="ts">
  import { api } from "../../api";
  import { attempt } from "../../actions";
  import { fileSize } from "../../format";
  import { t } from "../../i18n";
  import { settings } from "../../stores/settings.svelte";
  import { sidebar } from "../../stores/sidebar.svelte";
  import { articles } from "../../stores/app.svelte";
  import { toasts } from "../../stores/toasts.svelte";
  import type { Settings, StorageInfo } from "../../types";

  let { current }: { current: Settings } = $props();

  const RETENTION = [30, 90, 180, 365, 0] as const;
  const CACHE_SIZES = [100, 250, 500, 1000, 2000, 5000];

  let info = $state<StorageInfo | null>(null);
  let busy = $state<"cache" | "compact" | null>(null);

  async function load() {
    await attempt(async () => (info = await api.getStorageInfo()));
  }

  $effect(() => {
    void load();
  });

  async function clearCache() {
    busy = "cache";
    await attempt(async () => {
      await api.clearImageCache();
      toasts.info(t("settings.storage.cleared"));
    });
    busy = null;
    await load();
  }

  async function compact() {
    busy = "compact";
    await attempt(async () => {
      await api.compactDatabase();
      toasts.info(t("settings.storage.compacted"));
      sidebar.scheduleReload(0);
      await articles.load();
    });
    busy = null;
    await load();
  }
</script>

<div class="settings-page">
  <h3>{t("settings.storage.retention")}</h3>
  <label class="setting">
    <span>
      {t("settings.retention")}
      <span class="setting-hint">{t("settings.retention.hint")}</span>
    </span>
    <select
      value={current.retentionDays}
      onchange={(e) => settings.save({ retentionDays: Number(e.currentTarget.value) })}
    >
      {#each RETENTION as days (days)}
        <option value={days}>{t(`settings.retention.${days}`)}</option>
      {/each}
    </select>
  </label>

  <h3>{t("settings.storage.usage")}</h3>
  <dl class="usage">
    <dt>{t("settings.storage.database")}</dt>
    <dd>
      {info ? fileSize(info.databaseBytes) : "…"}
      {#if info}
        <span class="setting-hint"
          >{t("settings.storage.articles", { count: info.articleCount })}</span
        >
      {/if}
    </dd>
    <dt>{t("settings.storage.images")}</dt>
    <dd>{info ? fileSize(info.imageCacheBytes) : "…"}</dd>
  </dl>
  <label class="setting">
    <span>{t("settings.imageCacheLimit")}</span>
    <select
      value={current.imageCacheMb}
      onchange={(e) => settings.save({ imageCacheMb: Number(e.currentTarget.value) })}
    >
      {#each CACHE_SIZES as mb (mb)}
        <option value={mb}>{fileSize(mb * 1_000_000)}</option>
      {/each}
    </select>
  </label>
  <div class="setting-actions">
    <button class="button" onclick={clearCache} disabled={busy !== null}>
      {busy === "cache" ? t("settings.storage.working") : t("settings.storage.clearCache")}
    </button>
    <button class="button" onclick={compact} disabled={busy !== null}>
      {busy === "compact" ? t("settings.storage.working") : t("settings.storage.compact")}
    </button>
  </div>
  {#if info}
    <p class="setting-hint">{t("settings.storage.location", { path: info.dataDir })}</p>
  {/if}
</div>

<style>
  .usage {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 24px;
    margin: 0 0 8px;
  }

  dt {
    color: var(--text-muted);
  }

  dd {
    display: flex;
    gap: 10px;
    align-items: baseline;
    margin: 0;
    font-variant-numeric: tabular-nums;
  }
</style>
