<script lang="ts">
  import { t } from "../../i18n";
  import { settings } from "../../stores/settings.svelte";
  import type { ReaderFont, Settings } from "../../types";

  let { current }: { current: Settings } = $props();

  const FONTS: ReaderFont[] = ["system", "sans", "serif"];

  // Sliders preview live and save when released.
  let fontSize = $derived(current.readerFontSize);
  let lineWidth = $derived(current.readerLineWidth);
  let lineHeight = $derived(current.readerLineHeight);
</script>

<div class="settings-page">
  <h3>{t("settings.reading.text")}</h3>
  <label class="setting">
    <span>{t("settings.readerFont")}</span>
    <select
      value={current.readerFont}
      onchange={(e) => settings.save({ readerFont: e.currentTarget.value as ReaderFont })}
    >
      {#each FONTS as font (font)}
        <option value={font}>{t(`settings.readerFont.${font}`)}</option>
      {/each}
    </select>
  </label>
  <label class="setting">
    <span>{t("settings.readerFontSize")} <output>{fontSize}px</output></span>
    <input
      type="range"
      min="12"
      max="32"
      bind:value={fontSize}
      onchange={() => settings.save({ readerFontSize: fontSize })}
    />
  </label>
  <label class="setting">
    <span
      >{t("settings.readerLineWidth")}
      <output>{t("settings.characters", { count: lineWidth })}</output></span
    >
    <input
      type="range"
      min="40"
      max="120"
      step="2"
      bind:value={lineWidth}
      onchange={() => settings.save({ readerLineWidth: lineWidth })}
    />
  </label>
  <label class="setting">
    <span>{t("settings.readerLineHeight")} <output>{(lineHeight / 100).toFixed(2)}</output></span>
    <input
      type="range"
      min="120"
      max="220"
      step="5"
      bind:value={lineHeight}
      onchange={() => settings.save({ readerLineHeight: lineHeight })}
    />
  </label>
  <div
    class="preview"
    style:font-size="{fontSize}px"
    style:line-height={lineHeight / 100}
    style:max-width="{lineWidth}ch"
  >
    <p>{t("settings.reading.preview")}</p>
  </div>

  <h3>{t("settings.reading.list")}</h3>
  <label class="setting-check">
    <input
      type="checkbox"
      checked={current.listThumbnails}
      onchange={(e) => settings.save({ listThumbnails: e.currentTarget.checked })}
    />
    <span>
      {t("settings.listThumbnails")}
      <span class="setting-hint">{t("settings.listThumbnails.hint")}</span>
    </span>
  </label>
</div>

<style>
  output {
    color: var(--text-muted);
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
  }

  .preview {
    margin-top: 12px;
    padding: 12px 16px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg);
    font-family: var(--reader-font);
  }

  .preview p {
    margin: 0;
  }
</style>
