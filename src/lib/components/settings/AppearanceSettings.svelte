<script lang="ts">
  import { api } from "../../api";
  import { attempt } from "../../actions";
  import { t } from "../../i18n";
  import { settings } from "../../stores/settings.svelte";
  import { toasts } from "../../stores/toasts.svelte";
  import { applyCustomCss } from "../../theme";
  import type { AppInfo, Settings, Theme } from "../../types";

  let { current, info }: { current: Settings; info: AppInfo | null } = $props();

  const THEMES: Theme[] = ["system", "light", "dark"];
  const SCALES = [80, 90, 100, 110, 125, 150];
  /** Accent presets chosen to keep AA contrast for text on both themes' backgrounds. */
  const ACCENTS = ["#2f6fde", "#0f7d63", "#7b4fd6", "#c02d5b", "#b4461b", "#9a6700", "#3d5a80"];

  let custom = $derived(current.accentColor ?? "#2f6fde");

  async function reloadCustomCss() {
    await attempt(async () => {
      const css = await api.getCustomCss();
      applyCustomCss(css, document);
      toasts.info(css ? t("settings.customCss.loaded") : t("settings.customCss.none"));
    });
  }
</script>

<div class="settings-page">
  <h3>{t("settings.appearance.theme")}</h3>
  <div class="choices" role="radiogroup" aria-label={t("theme.label")}>
    {#each THEMES as theme (theme)}
      <label class="choice">
        <input
          type="radio"
          name="theme"
          checked={current.theme === theme}
          onchange={() => settings.save({ theme })}
        />
        {t(`theme.${theme}`)}
      </label>
    {/each}
  </div>

  <h3>{t("settings.accent")}</h3>
  <div class="swatches" role="radiogroup" aria-label={t("settings.accent")}>
    <label class="swatch default" title={t("settings.accent.default")}>
      <input
        type="radio"
        name="accent"
        checked={current.accentColor === null}
        onchange={() => settings.save({ accentColor: null })}
      />
      <span class="visually-hidden">{t("settings.accent.default")}</span>
    </label>
    {#each ACCENTS as color (color)}
      <label class="swatch" style:background={color} title={color}>
        <input
          type="radio"
          name="accent"
          checked={current.accentColor === color}
          onchange={() => settings.save({ accentColor: color })}
        />
        <span class="visually-hidden">{color}</span>
      </label>
    {/each}
    <label class="custom">
      <input
        type="color"
        bind:value={custom}
        onchange={() => settings.save({ accentColor: custom.toLowerCase() })}
      />
      {t("settings.accent.custom")}
    </label>
  </div>

  <h3>{t("settings.appearance.size")}</h3>
  <label class="setting">
    <span>{t("settings.uiScale")}</span>
    <select
      value={current.uiScale}
      onchange={(e) => settings.save({ uiScale: Number(e.currentTarget.value) })}
    >
      {#each SCALES as scale (scale)}
        <option value={scale}>{scale}%</option>
      {/each}
    </select>
  </label>
  <p class="setting-hint">{t("settings.appearance.resize")}</p>

  <h3>{t("settings.customCss")}</h3>
  <p class="setting-hint">
    {t("settings.customCss.hint", { path: info ? `${info.configDir}/custom.css` : "custom.css" })}
  </p>
  <div class="setting-actions">
    <button class="button" onclick={reloadCustomCss}>{t("settings.customCss.reload")}</button>
  </div>
</div>

<style>
  .choices {
    display: flex;
    gap: 8px;
  }

  .choice {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 6px 12px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
  }

  .choice:has(input:checked) {
    border-color: var(--accent);
    background: var(--bg-selected);
  }

  .swatches {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 8px;
  }

  .swatch {
    position: relative;
    width: 26px;
    height: 26px;
    border: 2px solid var(--bg);
    border-radius: 50%;
    outline: 1px solid var(--border);
    cursor: pointer;
  }

  .swatch.default {
    background: linear-gradient(135deg, #2f6fde 50%, #8fb4ff 50%);
  }

  .swatch:has(input:checked) {
    outline: 2px solid var(--text);
  }

  .swatch:has(input:focus-visible) {
    outline: 2px solid var(--focus-ring);
    outline-offset: 2px;
  }

  .swatch input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .custom {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-left: 8px;
    font-size: 0.88rem;
  }

  .custom input {
    width: 32px;
    height: 26px;
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 4px;
    background: none;
  }
</style>
