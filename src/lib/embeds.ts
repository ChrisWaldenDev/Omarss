// Click-to-load video embeds and images in the reader (SPEC §8.2, §8.4). The backend turns
// YouTube/Vimeo iframes into `<figure class="omarss-embed" data-embed="…">` placeholders and,
// with "Load remote images: Never", moves image addresses into `data-omarss-*` attributes.

/** Only these players may be loaded (the CSP's `frame-src` allows the same). */
const EMBED_PREFIXES = [
  "https://www.youtube-nocookie.com/embed/",
  "https://player.vimeo.com/video/",
];

export function isAllowedEmbed(src: string): boolean {
  return (
    EMBED_PREFIXES.some((prefix) => src.startsWith(prefix)) &&
    /^[A-Za-z0-9_-]+$/.test(src.slice(src.lastIndexOf("/") + 1))
  );
}

/** Adds a "Play" button to each embed placeholder; clicking it swaps in the player. */
export function enhanceEmbeds(root: HTMLElement, label: (host: string) => string): void {
  for (const figure of root.querySelectorAll<HTMLElement>("figure.omarss-embed[data-embed]")) {
    if (figure.querySelector(".omarss-play")) continue;
    const src = figure.dataset.embed ?? "";
    if (!isAllowedEmbed(src)) continue;
    const button = document.createElement("button");
    button.type = "button";
    button.className = "omarss-play";
    button.textContent = label(new URL(src).hostname);
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const frame = document.createElement("iframe");
      frame.src = src;
      frame.title = figure.textContent?.trim() || "Video";
      frame.allow = "autoplay; encrypted-media; fullscreen; picture-in-picture";
      frame.allowFullscreen = true;
      frame.referrerPolicy = "strict-origin-when-cross-origin";
      frame.setAttribute("sandbox", "allow-scripts allow-same-origin allow-presentation");
      // Keep the link to the video's page: some players refuse to play without a web
      // referrer, which the app's own origin can't provide on every platform.
      const link = figure.querySelector("a[href]");
      figure.replaceChildren(frame, ...(link ? [link] : []));
      figure.classList.add("loaded");
    });
    figure.prepend(button);
  }
}

/** Loads images whose addresses wait in `data-omarss-*` attributes. */
export function loadBlockedImages(root: HTMLElement): void {
  for (const name of ["src", "srcset", "poster"]) {
    const attribute = `data-omarss-${name}`;
    for (const element of root.querySelectorAll<HTMLElement>(`[${attribute}]`)) {
      element.setAttribute(name, element.getAttribute(attribute) ?? "");
      element.removeAttribute(attribute);
    }
  }
}
