import type { Theme } from "./types";

/**
 * Applies a theme to the document. "system" removes the override so the
 * `prefers-color-scheme` media query in app.css decides (SPEC §6.8).
 */
export function applyTheme(theme: Theme, root: { dataset: DOMStringMap }): void {
  if (theme === "system") delete root.dataset.theme;
  else root.dataset.theme = theme;
}
