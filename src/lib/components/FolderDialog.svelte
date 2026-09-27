<script lang="ts">
  import { api, errorMessage } from "../api";
  import { t } from "../i18n";
  import { sidebar } from "../stores/sidebar.svelte";
  import Dialog from "./Dialog.svelte";

  let {
    folderId,
    name: initialName,
    onclose,
  }: { folderId: number | null; name: string; onclose: () => void } = $props();

  // svelte-ignore state_referenced_locally
  let name = $state(initialName);
  let error = $state<string | null>(null);
  let saving = $state(false);

  async function save() {
    saving = true;
    error = null;
    try {
      if (folderId === null) await api.createFolder(name);
      else await api.renameFolder(folderId, name);
      onclose();
      await sidebar.load();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      saving = false;
    }
  }
</script>

<Dialog title={folderId === null ? t("folder.createTitle") : t("folder.renameTitle")} {onclose}>
  {#if error}
    <p class="form-error" role="alert">{error}</p>
  {/if}
  <form
    id="folder-form"
    onsubmit={(e) => {
      e.preventDefault();
      save();
    }}
  >
    <label class="field">
      <span>{t("folder.name")}</span>
      <!-- svelte-ignore a11y_autofocus -->
      <input type="text" bind:value={name} autofocus />
    </label>
  </form>
  {#snippet footer()}
    <button class="button" onclick={onclose}>{t("dialog.cancel")}</button>
    <button
      class="button primary"
      type="submit"
      form="folder-form"
      disabled={saving || !name.trim()}
    >
      {folderId === null ? t("folder.create") : t("dialog.save")}
    </button>
  {/snippet}
</Dialog>
