<script lang="ts">
  import { errorMessage } from "../../api";
  import { t } from "../../i18n";
  import { settings } from "../../stores/settings.svelte";
  import { toasts } from "../../stores/toasts.svelte";
  import type { LoadImages, Settings } from "../../types";

  let { current }: { current: Settings } = $props();

  const IMAGE_MODES: LoadImages[] = ["always", "opened", "never"];

  let proxy = $derived(current.proxyUrl);
  let proxyError = $state<string | null>(null);
  let saving = $state(false);

  async function saveProxy() {
    saving = true;
    proxyError = null;
    try {
      await settings.update({ proxyUrl: proxy.trim() });
      toasts.info(t("settings.proxy.saved"));
    } catch (error) {
      proxyError = errorMessage(error);
    } finally {
      saving = false;
    }
  }
</script>

<div class="settings-page">
  <h3>{t("settings.images")}</h3>
  {#each IMAGE_MODES as mode (mode)}
    <label class="setting-check">
      <input
        type="radio"
        name="load-images"
        checked={current.loadImages === mode}
        onchange={() => settings.save({ loadImages: mode })}
      />
      <span>
        {t(`settings.images.${mode}`)}
        <span class="setting-hint">{t(`settings.images.${mode}.hint`)}</span>
      </span>
    </label>
  {/each}
  <p class="setting-hint">{t("settings.images.proxy")}</p>

  <h3>{t("settings.privacy.links")}</h3>
  <label class="setting-check">
    <input
      type="checkbox"
      checked={current.stripTrackingParams}
      onchange={(e) => settings.save({ stripTrackingParams: e.currentTarget.checked })}
    />
    <span>
      {t("settings.stripTracking")}
      <span class="setting-hint">{t("settings.stripTracking.hint")}</span>
    </span>
  </label>

  <h3>{t("settings.proxy")}</h3>
  <p class="setting-hint">{t("settings.proxy.hint")}</p>
  {#if proxyError}
    <p class="form-error" role="alert">{proxyError}</p>
  {/if}
  <form
    class="proxy"
    onsubmit={(e) => {
      e.preventDefault();
      saveProxy();
    }}
  >
    <input
      type="text"
      bind:value={proxy}
      placeholder="socks5://127.0.0.1:1080"
      aria-label={t("settings.proxy")}
      spellcheck="false"
    />
    <button class="button" type="submit" disabled={saving || proxy.trim() === current.proxyUrl}>
      {t("settings.proxy.apply")}
    </button>
  </form>
</div>

<style>
  .proxy {
    display: flex;
    gap: 8px;
    padding: 8px 0;
  }
</style>
