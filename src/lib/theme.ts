import type { ReaderFont, Settings, Theme } from "./types";

/**
 * Applies a theme to the document. "system" removes the override so the
 * `prefers-color-scheme` media query in app.css decides (SPEC §6.8).
 */
export function applyTheme(theme: Theme, root: { dataset: DOMStringMap }): void {
  if (theme === "system") delete root.dataset.theme;
  else root.dataset.theme = theme;
}

/** Where appearance settings are written: `document.documentElement.style` in the app. */
export interface StyleTarget {
  setProperty(name: string, value: string): void;
  removeProperty(name: string): string;
}

export const READER_FONTS: Record<ReaderFont, string> = {
  system: "var(--font-ui)",
  sans: '"Inter", "Noto Sans", "Segoe UI", "Helvetica Neue", Arial, sans-serif',
  serif: 'Charter, "Bitstream Charter", "Noto Serif", "Source Serif Pro", Georgia, Cambria, serif',
};

const ACCENT_PROPERTIES = [
  "--accent",
  "--accent-text",
  "--accent-contrast",
  "--focus-ring",
  "--bg-selected",
];

/** Readable text colour on a `#rrggbb` background (WCAG relative luminance). */
export function contrastText(hex: string): "#000000" | "#ffffff" {
  const channel = (i: number) => {
    const c = parseInt(hex.slice(i, i + 2), 16) / 255;
    return c <= 0.03928 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4;
  };
  const luminance = 0.2126 * channel(1) + 0.7152 * channel(3) + 0.0722 * channel(5);
  // Pick whichever of black or white contrasts more.
  return (luminance + 0.05) / 0.05 > 1.05 / (luminance + 0.05) ? "#000000" : "#ffffff";
}

/**
 * Applies reader typography and the accent colour (SPEC §6.2, §6.8) as CSS custom
 * properties, so custom CSS can still override them.
 */
export function applyAppearance(settings: Settings, style: StyleTarget): void {
  style.setProperty("--reader-font", READER_FONTS[settings.readerFont]);
  style.setProperty("--reader-font-size", `${settings.readerFontSize}px`);
  style.setProperty("--reader-line-height", String(settings.readerLineHeight / 100));
  style.setProperty("--reader-width", `${settings.readerLineWidth}ch`);
  const accent = settings.accentColor;
  if (accent) {
    style.setProperty("--accent", accent);
    style.setProperty("--accent-text", accent);
    style.setProperty("--focus-ring", accent);
    style.setProperty("--accent-contrast", contrastText(accent));
    style.setProperty("--bg-selected", `color-mix(in srgb, ${accent} 18%, var(--bg))`);
  } else {
    for (const property of ACCENT_PROPERTIES) style.removeProperty(property);
  }
}

/** Installs (or replaces) the user's `custom.css`, after the app's own styles. */
export function applyCustomCss(css: string | null, doc: Document): void {
  const id = "omarss-custom-css";
  doc.getElementById(id)?.remove();
  if (!css) return;
  const style = doc.createElement("style");
  style.id = id;
  style.textContent = css;
  doc.head.append(style);
}
