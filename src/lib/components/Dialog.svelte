<script lang="ts">
  import type { Snippet } from "svelte";

  import { t } from "../i18n";
  import Icon from "./Icon.svelte";

  let {
    title,
    onclose,
    children,
    footer,
    wide = false,
  }: {
    title: string;
    onclose: () => void;
    children: Snippet;
    footer?: Snippet;
    wide?: boolean;
  } = $props();

  let dialog = $state<HTMLDialogElement>();
  const id = `dialog-${Math.random().toString(36).slice(2)}`;
  // Closing because this component is going away (e.g. another dialog replaced it) must not
  // report a close, or the replacement would be closed too.
  let unmounting = false;

  $effect(() => {
    dialog?.showModal();
    return () => {
      unmounting = true;
      dialog?.close();
    };
  });
</script>

<dialog
  bind:this={dialog}
  class:wide
  aria-labelledby={id}
  onclose={() => {
    if (!unmounting) onclose();
  }}
>
  <header>
    <h2 {id}>{title}</h2>
    <button class="close" aria-label={t("dialog.close")} onclick={() => dialog?.close()}>
      <Icon name="close" />
    </button>
  </header>
  <div class="body">
    {@render children()}
  </div>
  {#if footer}
    <footer>{@render footer()}</footer>
  {/if}
</dialog>

<style>
  dialog {
    width: min(30rem, calc(100vw - 32px));
    max-height: calc(100vh - 64px);
    padding: 0;
    border: 1px solid var(--border);
    border-radius: 10px;
    background: var(--bg);
    color: var(--text);
    box-shadow: var(--shadow);
  }

  dialog.wide {
    width: min(38rem, calc(100vw - 32px));
  }

  dialog::backdrop {
    background: rgb(0 0 0 / 0.35);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 14px 16px 6px 20px;
  }

  h2 {
    margin: 0;
    font-size: 1.05rem;
  }

  .close {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    padding: 0;
    border: 0;
    border-radius: var(--radius);
    background: none;
    color: var(--text-muted);
  }

  .close:hover {
    background: var(--bg-hover);
  }

  .body {
    padding: 8px 20px 16px;
  }

  footer {
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    padding: 12px 20px 16px;
    border-top: 1px solid var(--border-subtle);
  }
</style>
