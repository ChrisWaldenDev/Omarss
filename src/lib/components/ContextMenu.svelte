<script lang="ts" module>
  export interface MenuItem {
    label: string;
    action: () => void;
    danger?: boolean;
  }
</script>

<script lang="ts">
  let { x, y, items, onclose }: { x: number; y: number; items: MenuItem[]; onclose: () => void } =
    $props();

  let menu = $state<HTMLElement>();
  let position = $state({ left: 0, top: 0 });

  // Keep the menu on screen and focus its first item.
  $effect(() => {
    if (!menu) return;
    const { width, height } = menu.getBoundingClientRect();
    position = {
      left: Math.max(4, Math.min(x, window.innerWidth - width - 4)),
      top: Math.max(4, Math.min(y, window.innerHeight - height - 4)),
    };
    menu.querySelector<HTMLElement>("button")?.focus();
  });

  function choose(item: MenuItem) {
    onclose();
    item.action();
  }

  function onkeydown(event: KeyboardEvent) {
    const buttons = [...(menu?.querySelectorAll<HTMLElement>("button") ?? [])];
    const index = buttons.indexOf(document.activeElement as HTMLElement);
    if (event.key === "Escape") onclose();
    else if (event.key === "ArrowDown") buttons[(index + 1) % buttons.length]?.focus();
    else if (event.key === "ArrowUp")
      buttons[(index - 1 + buttons.length) % buttons.length]?.focus();
    else return;
    event.preventDefault();
  }
</script>

<svelte:window
  onpointerdown={(e) => {
    if (menu && !menu.contains(e.target as Node)) onclose();
  }}
  onblur={onclose}
  onresize={onclose}
/>

<div
  class="menu"
  role="menu"
  tabindex="-1"
  bind:this={menu}
  style:left="{position.left}px"
  style:top="{position.top}px"
  {onkeydown}
>
  {#each items as item (item.label)}
    <button role="menuitem" class:danger={item.danger} onclick={() => choose(item)}>
      {item.label}
    </button>
  {/each}
</div>

<style>
  .menu {
    position: fixed;
    z-index: 50;
    display: flex;
    flex-direction: column;
    min-width: 180px;
    padding: 4px;
    border: 1px solid var(--border);
    border-radius: var(--radius);
    background: var(--bg-input);
    box-shadow: var(--shadow);
  }

  button {
    padding: 6px 10px;
    border: 0;
    border-radius: calc(var(--radius) - 2px);
    background: none;
    text-align: left;
  }

  button:hover,
  button:focus-visible {
    background: var(--bg-selected);
    outline: none;
  }

  .danger {
    color: var(--danger);
  }
</style>
