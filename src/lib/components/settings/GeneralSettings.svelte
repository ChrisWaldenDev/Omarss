<script lang="ts">
  import { exportOpml, importOpml } from "../../actions";
  import { t } from "../../i18n";
  import { settings } from "../../stores/settings.svelte";
  import type { Layout, MarkReadMode, Settings } from "../../types";

  let { current }: { current: Settings } = $props();

  const LAYOUTS: Layout[] = ["threePane", "twoPane"];
  const MODES: MarkReadMode[] = ["onOpen", "afterDelay", "onScroll"];
  const DELAYS = [1, 2, 3, 5, 10, 15, 30, 60];
</script>

<div class="settings-page">
  <h3>{t("settings.general.layout")}</h3>
  {#each LAYOUTS as layout (layout)}
    <label class="setting-check">
      <input
        type="radio"
        name="layout"
        checked={current.layout === layout}
        onchange={() => settings.save({ layout })}
      />
      <span>
        {t(`settings.layout.${layout}`)}
        <span class="setting-hint">{t(`settings.layout.${layout}.hint`)}</span>
      </span>
    </label>
  {/each}

  <h3>{t("settings.general.reading")}</h3>
  <label class="setting">
    <span>{t("settings.markRead")}</span>
    <select
      value={current.markReadMode}
      onchange={(e) => settings.save({ markReadMode: e.currentTarget.value as MarkReadMode })}
    >
      {#each MODES as mode (mode)}
        <option value={mode}>{t(`settings.markRead.${mode}`)}</option>
      {/each}
    </select>
  </label>
  {#if current.markReadMode === "afterDelay"}
    <label class="setting">
      <span>{t("settings.markReadDelay")}</span>
      <select
        value={current.markReadDelaySeconds}
        onchange={(e) => settings.save({ markReadDelaySeconds: Number(e.currentTarget.value) })}
      >
        {#each DELAYS as seconds (seconds)}
          <option value={seconds}>{t("settings.seconds", { count: seconds })}</option>
        {/each}
      </select>
    </label>
  {/if}
  <label class="setting-check">
    <input
      type="checkbox"
      checked={current.markUpdatedUnread}
      onchange={(e) => settings.save({ markUpdatedUnread: e.currentTarget.checked })}
    />
    <span>
      {t("settings.markUpdatedUnread")}
      <span class="setting-hint">{t("settings.markUpdatedUnread.hint")}</span>
    </span>
  </label>

  <h3>{t("settings.general.subscriptions")}</h3>
  <p class="setting-hint">{t("settings.opml.hint")}</p>
  <div class="setting-actions">
    <button class="button" onclick={importOpml}>{t("menu.importOpml")}</button>
    <button class="button" onclick={exportOpml}>{t("menu.exportOpml")}</button>
  </div>
</div>
