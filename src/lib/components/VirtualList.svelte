<script lang="ts" generics="T">
  import type { Snippet } from "svelte";

  import { scrollToReveal, visibleRange } from "../virtual";

  let {
    items,
    rowHeight,
    key,
    row,
    onEndReached,
    revealIndex = null,
    label,
  }: {
    items: T[];
    rowHeight: number;
    key: (item: T) => number | string;
    row: Snippet<[T, number]>;
    /** Called when the last rows come into view (load the next page). */
    onEndReached?: () => void;
    /** Scrolls this row into view when it changes. */
    revealIndex?: number | null;
    label?: string;
  } = $props();

  let viewport = $state<HTMLElement>();
  let scrollTop = $state(0);
  let height = $state(0);

  const range = $derived(visibleRange(scrollTop, height, rowHeight, items.length));
  const visible = $derived(items.slice(range.start, range.end));

  $effect(() => {
    if (onEndReached && items.length > 0 && range.end >= items.length - 10) onEndReached();
  });

  $effect(() => {
    if (revealIndex === null || !viewport) return;
    const target = scrollToReveal(
      revealIndex,
      viewport.scrollTop,
      viewport.clientHeight,
      rowHeight,
    );
    if (target !== null) viewport.scrollTop = target;
  });

  /** Jumps back to the top (e.g. after switching views). */
  export function scrollToTop(): void {
    if (viewport) viewport.scrollTop = 0;
  }
</script>

<div
  class="viewport"
  bind:this={viewport}
  bind:clientHeight={height}
  onscroll={() => (scrollTop = viewport?.scrollTop ?? 0)}
  role="list"
  aria-label={label}
>
  <div class="spacer" style:height="{items.length * rowHeight}px">
    <div class="window" style:transform="translateY({range.offset}px)">
      {#each visible as item, i (key(item))}
        <div class="virtual-row" role="listitem" style:height="{rowHeight}px">
          {@render row(item, range.start + i)}
        </div>
      {/each}
    </div>
  </div>
</div>

<style>
  .viewport {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    overscroll-behavior: contain;
  }

  .spacer {
    position: relative;
  }

  .window {
    position: absolute;
    top: 0;
    right: 0;
    left: 0;
  }

  .virtual-row {
    overflow: hidden;
  }
</style>
