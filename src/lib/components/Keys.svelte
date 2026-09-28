<script lang="ts">
  import { t } from "../i18n";
  import { keyParts } from "../keyboard";

  let { binding }: { binding: string } = $props();

  const NAMES: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Escape: "Esc",
    ArrowUp: "↑",
    ArrowDown: "↓",
    ArrowLeft: "←",
    ArrowRight: "→",
  };
</script>

<span class="keys">
  {#each keyParts(binding) as step, i (i)}
    {#if i > 0}<span class="then">{t("shortcuts.then")}</span>{/if}
    {#each step as part, j (j)}
      {#if j > 0}<span class="plus">+</span>{/if}<kbd>{NAMES[part] ?? part}</kbd>
    {/each}
  {/each}
</span>

<style>
  .keys {
    display: inline-flex;
    flex-wrap: wrap;
    align-items: center;
    gap: 3px;
  }

  kbd {
    min-width: 1.6em;
    padding: 1px 6px;
    border: 1px solid var(--border);
    border-bottom-width: 2px;
    border-radius: 4px;
    background: var(--bg-input);
    font-family: var(--font-mono);
    font-size: 0.8rem;
    text-align: center;
  }

  .then,
  .plus {
    color: var(--text-muted);
    font-size: 0.75rem;
  }
</style>
