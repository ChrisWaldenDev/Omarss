<script lang="ts" module>
  // Simple 24×24 stroke icons. Static markup only; never pass user content here.
  const ICONS = {
    all: '<path d="M12 3 3 8l9 5 9-5-9-5z"/><path d="m3 13 9 5 9-5"/>',
    unread: '<circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="3" fill="currentColor"/>',
    star: '<path d="m12 3 2.8 5.9 6.2.7-4.6 4.3 1.2 6.3L12 17l-5.6 3.2 1.2-6.3L3 9.6l6.2-.7z"/>',
    today: '<rect x="3" y="5" width="18" height="16" rx="2"/><path d="M3 10h18M8 3v4M16 3v4"/>',
    folder:
      '<path d="M3 6a1 1 0 0 1 1-1h5l2 2h9a1 1 0 0 1 1 1v10a1 1 0 0 1-1 1H4a1 1 0 0 1-1-1z"/>',
    chevron: '<path d="m9 6 6 6-6 6"/>',
    warning: '<path d="M12 3 2 20h20z"/><path d="M12 10v4M12 17h.01"/>',
    external:
      '<path d="M14 4h6v6M20 4l-9 9"/><path d="M18 14v5a1 1 0 0 1-1 1H5a1 1 0 0 1-1-1V7a1 1 0 0 1 1-1h5"/>',
    system: '<rect x="3" y="4" width="18" height="12" rx="1"/><path d="M8 20h8M12 16v4"/>',
    light:
      '<circle cx="12" cy="12" r="4"/><path d="M12 2v2M12 20v2M4.9 4.9l1.4 1.4M17.7 17.7l1.4 1.4M2 12h2M20 12h2M4.9 19.1l1.4-1.4M17.7 6.3l1.4-1.4"/>',
    dark: '<path d="M20 14.5A8 8 0 1 1 9.5 4a6.5 6.5 0 0 0 10.5 10.5z"/>',
    sort: '<path d="M7 4v16M3 16l4 4 4-4M17 20V4M13 8l4-4 4 4"/>',
    attachment:
      '<path d="m20 11-8 8a5 5 0 0 1-7-7l8.5-8.5a3.5 3.5 0 0 1 5 5L10 17a2 2 0 0 1-3-3l7.5-7.5"/>',
  } as const;

  export type IconName = keyof typeof ICONS;
</script>

<script lang="ts">
  let {
    name,
    size = 16,
    filled = false,
  }: { name: IconName; size?: number; filled?: boolean } = $props();
</script>

<svg
  class="icon"
  width={size}
  height={size}
  viewBox="0 0 24 24"
  fill={filled ? "currentColor" : "none"}
  stroke="currentColor"
  stroke-width="2"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
  focusable="false"
>
  <!-- eslint-disable-next-line svelte/no-at-html-tags -- static icon markup from ICONS -->
  {@html ICONS[name]}
</svg>

<style>
  .icon {
    flex: none;
  }
</style>
