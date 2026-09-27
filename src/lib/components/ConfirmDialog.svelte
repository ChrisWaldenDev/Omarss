<script lang="ts">
  import { errorMessage } from "../api";
  import { t } from "../i18n";
  import Dialog from "./Dialog.svelte";

  let {
    title,
    message,
    confirmLabel,
    onConfirm,
    onclose,
  }: {
    title: string;
    message: string;
    confirmLabel: string;
    onConfirm: () => Promise<void>;
    onclose: () => void;
  } = $props();

  let busy = $state(false);
  let error = $state<string | null>(null);

  async function confirm() {
    busy = true;
    error = null;
    try {
      await onConfirm();
      onclose();
    } catch (e) {
      error = errorMessage(e);
    } finally {
      busy = false;
    }
  }
</script>

<Dialog {title} {onclose}>
  {#if error}
    <p class="form-error" role="alert">{error}</p>
  {/if}
  <p class="message">{message}</p>
  {#snippet footer()}
    <!-- svelte-ignore a11y_autofocus -->
    <button class="button" onclick={onclose} autofocus>{t("dialog.cancel")}</button>
    <button class="button danger" onclick={confirm} disabled={busy}>{confirmLabel}</button>
  {/snippet}
</Dialog>

<style>
  .message {
    margin: 0;
    line-height: 1.5;
  }
</style>
