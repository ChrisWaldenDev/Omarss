<script lang="ts">
  // A draggable (and keyboard-adjustable) divider that resizes the pane before it
  // (SPEC §6.8: sidebar and list widths are resizable and persisted).

  let {
    value,
    min,
    max,
    label,
    onchange,
    oncommit,
  }: {
    /** Width of the pane before the divider, in pixels. */
    value: number;
    min: number;
    max: number;
    label: string;
    /** Called while dragging. */
    onchange: (width: number) => void;
    /** Called when a resize ends, to save it. */
    oncommit: (width: number) => void;
  } = $props();

  const STEP = 16;
  let drag = $state<{ startX: number; startWidth: number } | null>(null);

  const clamp = (width: number) => Math.round(Math.min(max, Math.max(min, width)));

  function onpointerdown(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    drag = { startX: event.clientX, startWidth: value };
  }

  function onpointermove(event: PointerEvent) {
    if (drag) onchange(clamp(drag.startWidth + event.clientX - drag.startX));
  }

  function onpointerup() {
    if (!drag) return;
    drag = null;
    oncommit(value);
  }

  function onkeydown(event: KeyboardEvent) {
    const next =
      event.key === "ArrowLeft"
        ? value - STEP
        : event.key === "ArrowRight"
          ? value + STEP
          : event.key === "Home"
            ? min
            : event.key === "End"
              ? max
              : null;
    if (next === null) return;
    event.preventDefault();
    event.stopPropagation();
    onchange(clamp(next));
    oncommit(clamp(next));
  }
</script>

<!-- svelte-ignore a11y_no_noninteractive_tabindex, a11y_no_noninteractive_element_interactions -->
<div
  class="splitter"
  class:dragging={drag !== null}
  role="separator"
  aria-orientation="vertical"
  aria-label={label}
  aria-valuenow={value}
  aria-valuemin={min}
  aria-valuemax={max}
  tabindex="0"
  {onpointerdown}
  {onpointermove}
  {onpointerup}
  onpointercancel={onpointerup}
  {onkeydown}
></div>

<style>
  .splitter {
    position: relative;
    z-index: 5;
    width: 0;
    cursor: col-resize;
    touch-action: none;
  }

  /* A wider, invisible grab area centred on the pane border. */
  .splitter::before {
    content: "";
    position: absolute;
    inset: 0 -4px;
  }

  .splitter:hover::before,
  .splitter.dragging::before,
  .splitter:focus-visible::before {
    inset: 0 -2px;
    background: var(--accent);
    opacity: 0.6;
  }

  .splitter:focus-visible {
    outline: none;
  }
</style>
