// Sharing an article's link (SPEC §6.2): copy it, copy it as Markdown, or email it.

/** `[Title](url)` with the characters Markdown would misread escaped. */
export function markdownLink(title: string, url: string): string {
  const text = title.replace(/([\\[\]])/g, "\\$1");
  const target = url.replace(
    /[()\s]/g,
    (c) => `%${c.charCodeAt(0).toString(16).toUpperCase().padStart(2, "0")}`,
  );
  return `[${text}](${target})`;
}

export function mailtoLink(title: string, url: string): string {
  return `mailto:?subject=${encodeURIComponent(title)}&body=${encodeURIComponent(url)}`;
}

/** Copies text, falling back to a hidden text area where the Clipboard API is unavailable. */
export async function copyText(text: string): Promise<void> {
  try {
    await navigator.clipboard.writeText(text);
    return;
  } catch {
    // Not allowed or unsupported in this WebView: use the older approach.
  }
  const area = document.createElement("textarea");
  area.value = text;
  area.setAttribute("readonly", "");
  area.style.position = "fixed";
  area.style.opacity = "0";
  document.body.append(area);
  area.select();
  const copied = document.execCommand("copy");
  area.remove();
  if (!copied) throw new Error("The clipboard isn't available");
}
