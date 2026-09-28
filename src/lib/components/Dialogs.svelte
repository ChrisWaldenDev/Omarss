<script lang="ts">
  import { dialogs } from "../stores/dialogs.svelte";
  import AddFeedDialog from "./AddFeedDialog.svelte";
  import ConfirmDialog from "./ConfirmDialog.svelte";
  import EditFeedDialog from "./EditFeedDialog.svelte";
  import FolderDialog from "./FolderDialog.svelte";
  import SettingsDialog from "./SettingsDialog.svelte";
  import ShortcutsDialog from "./ShortcutsDialog.svelte";

  const close = () => dialogs.close();
</script>

{#if dialogs.current?.kind === "addFeed"}
  <AddFeedDialog onclose={close} />
{:else if dialogs.current?.kind === "editFeed"}
  {#key dialogs.current.feedId}
    <EditFeedDialog
      feedId={dialogs.current.feedId}
      focusUrl={dialogs.current.focusUrl ?? false}
      onclose={close}
    />
  {/key}
{:else if dialogs.current?.kind === "settings"}
  <SettingsDialog section={dialogs.current.section} onclose={close} />
{:else if dialogs.current?.kind === "shortcuts"}
  <ShortcutsDialog onclose={close} />
{:else if dialogs.current?.kind === "folder"}
  <FolderDialog folderId={dialogs.current.folderId} name={dialogs.current.name} onclose={close} />
{:else if dialogs.current?.kind === "confirm"}
  {@const confirm = dialogs.current}
  <ConfirmDialog
    title={confirm.title}
    message={confirm.message}
    confirmLabel={confirm.confirmLabel}
    onConfirm={confirm.onConfirm}
    onclose={close}
  />
{/if}
