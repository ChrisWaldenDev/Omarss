<script lang="ts">
  // Rebinding shortcuts (SPEC §6.7). "Change" listens for the next key, or two keys in quick
  // succession for a sequence like "g a".
  import { t } from "../../i18n";
  import {
    actionLabel,
    conflictsWith,
    defaultBindings,
    eventToKey,
    overridesFrom,
    resolveBindings,
    type ActionId,
  } from "../../keyboard";
  import { SHORTCUT_GROUPS } from "../../shortcutGroups";
  import { settings } from "../../stores/settings.svelte";
  import type { Settings } from "../../types";
  import Keys from "../Keys.svelte";

  let { current }: { current: Settings } = $props();

  /** How long to wait for the second key of a sequence. */
  const SEQUENCE_MS = 900;

  const bindings = $derived(resolveBindings(current.shortcuts));
  const defaults = defaultBindings();

  let capturing = $state<ActionId | null>(null);
  let first = $state<string | null>(null);
  let message = $state<string | null>(null);
  let timer: ReturnType<typeof setTimeout> | null = null;

  function start(action: ActionId) {
    capturing = action;
    first = null;
    message = null;
  }

  function stop() {
    if (timer) clearTimeout(timer);
    timer = null;
    capturing = null;
    first = null;
  }

  async function save(action: ActionId, keys: string[]) {
    await settings.save({ shortcuts: overridesFrom({ ...bindings, [action]: keys }) });
  }

  async function commit(binding: string) {
    const action = capturing;
    stop();
    if (!action) return;
    const clash = conflictsWith(bindings, binding, action);
    if (clash) {
      message = t("settings.keyboard.conflict", { action: t(actionLabel(clash)) });
      return;
    }
    await save(action, [binding]);
  }

  function onkeydown(event: KeyboardEvent) {
    if (!capturing) return;
    event.preventDefault();
    event.stopPropagation();
    const key = eventToKey(event);
    if (!key) return;
    if (key === "Escape" && first === null) {
      stop();
      return;
    }
    if (timer) clearTimeout(timer);
    if (first === null) {
      first = key;
      const pending = key;
      timer = setTimeout(() => commit(pending), SEQUENCE_MS);
    } else {
      void commit(`${first} ${key}`);
    }
  }

  const same = (a: string[], b: string[]) => a.join("\n") === b.join("\n");
</script>

<svelte:window onkeydowncapture={onkeydown} />

<div class="settings-page">
  <p class="setting-hint">{t("settings.keyboard.hint")}</p>
  {#if message}
    <p class="form-error" role="alert">{message}</p>
  {/if}
  {#each SHORTCUT_GROUPS as group (group.title)}
    <h3>{t(group.title)}</h3>
    {#each group.actions as action (action)}
      <div class="setting">
        <span>{t(actionLabel(action))}</span>
        <span class="keys">
          {#if capturing === action}
            <span class="listening" aria-live="polite">
              {first ? t("settings.keyboard.second", { key: first }) : t("settings.keyboard.press")}
            </span>
          {:else}
            {#each bindings[action] as binding (binding)}
              <Keys {binding} />
            {:else}
              <span class="setting-hint">{t("shortcuts.unbound")}</span>
            {/each}
          {/if}
          <button
            class="button small"
            onclick={() => (capturing === action ? stop() : start(action))}
          >
            {capturing === action ? t("dialog.cancel") : t("settings.keyboard.change")}
          </button>
          {#if !same(bindings[action], defaults[action])}
            <button class="button small" onclick={() => save(action, defaults[action])}>
              {t("settings.keyboard.reset")}
            </button>
          {/if}
        </span>
      </div>
    {/each}
  {/each}
  <div class="setting-actions">
    <button
      class="button"
      disabled={Object.keys(current.shortcuts).length === 0}
      onclick={() => settings.save({ shortcuts: {} })}
    >
      {t("settings.keyboard.resetAll")}
    </button>
  </div>
</div>

<style>
  .keys {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 6px;
  }

  .listening {
    color: var(--accent-text);
    font-size: 0.85rem;
    font-weight: 600;
  }

  .small {
    padding: 2px 10px;
    font-size: 0.8rem;
  }
</style>
