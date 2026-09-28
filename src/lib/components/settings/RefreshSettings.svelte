<script lang="ts">
  import { t } from "../../i18n";
  import { settings } from "../../stores/settings.svelte";
  import type { AppInfo, Settings } from "../../types";

  let { current, info }: { current: Settings; info: AppInfo | null } = $props();

  const INTERVALS = [15, 30, 60, 120, 360, 0] as const;
</script>

<div class="settings-page">
  <h3>{t("settings.refresh.schedule")}</h3>
  <label class="setting">
    <span>
      {t("settings.refreshInterval")}
      <span class="setting-hint">{t("settings.refreshInterval.hint")}</span>
    </span>
    <select
      value={current.refreshIntervalMinutes}
      onchange={(e) => settings.save({ refreshIntervalMinutes: Number(e.currentTarget.value) })}
    >
      {#each INTERVALS as minutes (minutes)}
        <option value={minutes}>{t(`settings.refreshInterval.${minutes}`)}</option>
      {/each}
    </select>
  </label>
  <label class="setting-check">
    <input
      type="checkbox"
      checked={current.refreshOnStartup}
      onchange={(e) => settings.save({ refreshOnStartup: e.currentTarget.checked })}
    />
    <span>{t("settings.refreshOnStartup")}</span>
  </label>
  {#if info?.meteredSupported}
    <label class="setting-check">
      <input
        type="checkbox"
        checked={current.pauseOnMetered}
        onchange={(e) => settings.save({ pauseOnMetered: e.currentTarget.checked })}
      />
      <span>
        {t("settings.pauseOnMetered")}
        <span class="setting-hint">{t("settings.pauseOnMetered.hint")}</span>
      </span>
    </label>
  {/if}
</div>
