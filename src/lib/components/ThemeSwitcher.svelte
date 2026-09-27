<script lang="ts">
  import { errorMessage } from "../api";
  import { t } from "../i18n";
  import { settings } from "../stores/settings.svelte";
  import type { Theme } from "../types";
  import Icon from "./Icon.svelte";

  const THEMES: Theme[] = ["system", "light", "dark"];

  let error = $state<string | null>(null);

  async function choose(theme: Theme) {
    error = null;
    try {
      await settings.update({ theme });
    } catch (e) {
      error = t("settings.saveError", { message: errorMessage(e) });
    }
  }
</script>

<fieldset class="switcher">
  <legend class="visually-hidden">{t("theme.label")}</legend>
  {#each THEMES as theme (theme)}
    <label title={t(`theme.${theme}`)}>
      <input
        type="radio"
        name="theme"
        value={theme}
        checked={settings.current?.theme === theme}
        onchange={() => choose(theme)}
      />
      <Icon name={theme} />
      <span class="visually-hidden">{t(`theme.${theme}`)}</span>
    </label>
  {/each}
</fieldset>
{#if error}
  <p class="error" role="alert">{error}</p>
{/if}

<style>
  .switcher {
    display: inline-flex;
    gap: 2px;
    margin: 0;
    padding: 2px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-input);
  }

  label {
    position: relative;
    display: grid;
    place-items: center;
    width: 30px;
    height: 24px;
    border-radius: calc(var(--radius) - 2px);
    color: var(--text-muted);
    cursor: pointer;
  }

  label:hover {
    color: var(--text);
  }

  label:has(input:checked) {
    background: var(--bg-selected);
    color: var(--accent-text);
  }

  label:has(input:focus-visible) {
    outline: 2px solid var(--focus-ring);
    outline-offset: 1px;
  }

  input {
    position: absolute;
    opacity: 0;
    pointer-events: none;
  }

  .error {
    margin: 6px 0 0;
    color: var(--danger);
    font-size: 0.8rem;
  }
</style>
