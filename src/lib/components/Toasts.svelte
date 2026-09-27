<script lang="ts">
  import { t } from "../i18n";
  import { toasts } from "../stores/toasts.svelte";
  import Icon from "./Icon.svelte";
</script>

<div class="toasts" role="status" aria-live="polite">
  {#each toasts.items as toast (toast.id)}
    <div class="toast">
      <span>{toast.message}</span>
      <button aria-label={t("dialog.close")} onclick={() => toasts.dismiss(toast.id)}>
        <Icon name="close" size={14} />
      </button>
    </div>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 16px;
    bottom: 16px;
    z-index: 60;
    display: flex;
    flex-direction: column;
    gap: 8px;
    max-width: 24rem;
  }

  .toast {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 10px 10px 10px 14px;
    border: 1px solid var(--border);
    border-left: 3px solid var(--danger);
    border-radius: var(--radius);
    background: var(--bg-input);
    box-shadow: var(--shadow);
    font-size: 0.88rem;
  }

  button {
    display: grid;
    flex: none;
    place-items: center;
    padding: 2px;
    border: 0;
    background: none;
    color: var(--text-muted);
  }
</style>
