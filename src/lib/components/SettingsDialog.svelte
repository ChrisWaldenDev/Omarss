<script lang="ts">
  // Settings (SPEC §11.2). Every control saves straight away. Notifications, Accounts and
  // Rules sections arrive with their milestones.
  import { api } from "../api";
  import { t } from "../i18n";
  import type { SettingsSection } from "../stores/dialogs.svelte";
  import { settings } from "../stores/settings.svelte";
  import type { AppInfo } from "../types";
  import Dialog from "./Dialog.svelte";
  import AboutSettings from "./settings/AboutSettings.svelte";
  import AppearanceSettings from "./settings/AppearanceSettings.svelte";
  import GeneralSettings from "./settings/GeneralSettings.svelte";
  import KeyboardSettings from "./settings/KeyboardSettings.svelte";
  import PrivacySettings from "./settings/PrivacySettings.svelte";
  import ReadingSettings from "./settings/ReadingSettings.svelte";
  import RefreshSettings from "./settings/RefreshSettings.svelte";
  import StorageSettings from "./settings/StorageSettings.svelte";

  let { section = "general", onclose }: { section?: SettingsSection; onclose: () => void } =
    $props();

  const SECTIONS: SettingsSection[] = [
    "general",
    "reading",
    "refresh",
    "appearance",
    "keyboard",
    "privacy",
    "storage",
    "about",
  ];

  let active = $derived<SettingsSection>(section);
  let info = $state<AppInfo | null>(null);

  $effect(() => {
    api
      .getAppInfo()
      .then((result) => (info = result))
      .catch((error) => console.error("get_app_info failed", error));
  });

  function onNavKeydown(event: KeyboardEvent) {
    const index = SECTIONS.indexOf(active);
    const next = event.key === "ArrowDown" ? index + 1 : event.key === "ArrowUp" ? index - 1 : null;
    if (next === null) return;
    event.preventDefault();
    active = SECTIONS[(next + SECTIONS.length) % SECTIONS.length];
    (event.currentTarget as HTMLElement)
      .querySelector<HTMLElement>(`[data-section="${active}"]`)
      ?.focus();
  }
</script>

<Dialog title={t("settings.title")} {onclose} large>
  <div class="settings">
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <nav aria-label={t("settings.sections")} onkeydown={onNavKeydown}>
      {#each SECTIONS as item (item)}
        <button
          data-section={item}
          aria-current={active === item ? "page" : undefined}
          onclick={() => (active = item)}
        >
          {t(`settings.section.${item}`)}
        </button>
      {/each}
    </nav>
    <div class="page" role="region" aria-label={t(`settings.section.${active}`)}>
      {#if settings.current}
        {@const current = settings.current}
        {#if active === "general"}
          <GeneralSettings {current} />
        {:else if active === "reading"}
          <ReadingSettings {current} />
        {:else if active === "refresh"}
          <RefreshSettings {current} {info} />
        {:else if active === "appearance"}
          <AppearanceSettings {current} {info} />
        {:else if active === "keyboard"}
          <KeyboardSettings {current} />
        {:else if active === "privacy"}
          <PrivacySettings {current} />
        {:else if active === "storage"}
          <StorageSettings {current} />
        {:else}
          <AboutSettings {info} />
        {/if}
      {/if}
    </div>
  </div>
</Dialog>

<style>
  .settings {
    display: grid;
    grid-template-columns: 11rem minmax(0, 1fr);
    height: 100%;
    border-top: 1px solid var(--border-subtle);
  }

  nav {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 12px 8px;
    overflow-y: auto;
    border-right: 1px solid var(--border-subtle);
    background: var(--bg-sidebar);
  }

  nav button {
    padding: 6px 12px;
    border: 0;
    border-radius: var(--radius);
    background: none;
    text-align: left;
  }

  nav button:hover {
    background: var(--bg-hover);
  }

  nav button[aria-current="page"] {
    background: var(--bg-selected);
    color: var(--accent-text);
    font-weight: 600;
  }

  .page {
    padding: 16px 24px 24px;
    overflow-y: auto;
  }
</style>
