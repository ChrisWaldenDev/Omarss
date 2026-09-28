<script lang="ts">
  // The shortcut cheatsheet (`?`, SPEC §6.7), showing the user's current bindings.
  import { t } from "../i18n";
  import { actionLabel, resolveBindings } from "../keyboard";
  import { SHORTCUT_GROUPS } from "../shortcutGroups";
  import { dialogs } from "../stores/dialogs.svelte";
  import { settings } from "../stores/settings.svelte";
  import Dialog from "./Dialog.svelte";
  import Keys from "./Keys.svelte";

  let { onclose }: { onclose: () => void } = $props();

  const bindings = $derived(resolveBindings(settings.current?.shortcuts ?? {}));
</script>

<Dialog title={t("shortcuts.title")} {onclose} wide>
  <div class="groups">
    {#each SHORTCUT_GROUPS as group (group.title)}
      <section>
        <h3>{t(group.title)}</h3>
        <dl>
          {#each group.actions as action (action)}
            <div class="row">
              <dt>{t(actionLabel(action))}</dt>
              <dd>
                {#each bindings[action] as binding, i (binding)}
                  {#if i > 0}<span class="or">{t("shortcuts.or")}</span>{/if}
                  <Keys {binding} />
                {:else}
                  <span class="none">{t("shortcuts.unbound")}</span>
                {/each}
              </dd>
            </div>
          {/each}
          {#if group.title === "shortcuts.group.app"}
            <div class="row">
              <dt>{t("shortcuts.escape")}</dt>
              <dd><Keys binding="Escape" /></dd>
            </div>
          {/if}
        </dl>
      </section>
    {/each}
  </div>
  <p class="hint">
    {t("shortcuts.hint")}
    <button class="link" onclick={() => dialogs.open({ kind: "settings", section: "keyboard" })}>
      {t("shortcuts.customise")}
    </button>
  </p>
</Dialog>

<style>
  .groups {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(16rem, 1fr));
    gap: 4px 24px;
  }

  h3 {
    margin: 8px 0 6px;
    color: var(--text-muted);
    font-size: 0.75rem;
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  dl {
    margin: 0;
  }

  .row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
    padding: 4px 0;
    border-bottom: 1px solid var(--border-subtle);
  }

  dt {
    font-size: 0.88rem;
  }

  dd {
    display: flex;
    flex-wrap: wrap;
    align-items: center;
    justify-content: flex-end;
    gap: 4px;
    margin: 0;
  }

  .or,
  .none {
    color: var(--text-muted);
    font-size: 0.75rem;
  }

  .hint {
    margin: 14px 0 0;
    color: var(--text-muted);
    font-size: 0.82rem;
  }

  .link {
    padding: 0;
    border: 0;
    background: none;
    color: var(--accent-text);
    text-decoration: underline;
  }
</style>
