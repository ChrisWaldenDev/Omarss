<script lang="ts">
  import { openExternalLink } from "../../actions";
  import { t } from "../../i18n";
  import type { AppInfo } from "../../types";

  let { info }: { info: AppInfo | null } = $props();
</script>

<div class="settings-page about">
  <img src="/logo.svg" alt="" width="56" height="56" />
  <h3>Omarss</h3>
  {#if info}
    <p>{t("settings.about.version", { version: info.version, platform: info.platform })}</p>
  {/if}
  <p class="setting-hint">{t("settings.about.tagline")}</p>
  <p class="setting-hint">{t("settings.about.privacy")}</p>
  <p class="setting-hint">{t("settings.about.licence")}</p>
  {#if info}
    {@const homepage = info.homepage}
    <div class="setting-actions">
      <button class="button" onclick={() => openExternalLink(homepage)}>
        {t("settings.about.homepage")}
      </button>
    </div>
    <dl>
      <dt>{t("settings.about.data")}</dt>
      <dd><code>{info.dataDir}</code></dd>
      <dt>{t("settings.about.config")}</dt>
      <dd><code>{info.configDir}</code></dd>
    </dl>
  {/if}
</div>

<style>
  .about p {
    margin: 0 0 6px;
  }

  dl {
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 16px;
    margin: 16px 0 0;
    font-size: 0.85rem;
  }

  dt {
    color: var(--text-muted);
  }

  dd {
    margin: 0;
    overflow-wrap: anywhere;
  }
</style>
