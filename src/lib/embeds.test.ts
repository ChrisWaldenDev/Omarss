import { describe, expect, it } from "vitest";

import { isAllowedEmbed } from "./embeds";

describe("isAllowedEmbed", () => {
  it("allows only the privacy-friendly players the backend produces", () => {
    expect(isAllowedEmbed("https://www.youtube-nocookie.com/embed/dQw4w9WgXcQ")).toBe(true);
    expect(isAllowedEmbed("https://player.vimeo.com/video/76979871")).toBe(true);
    expect(isAllowedEmbed("https://www.youtube.com/embed/dQw4w9WgXcQ")).toBe(false);
    expect(isAllowedEmbed("https://www.youtube-nocookie.com/embed/x?autoplay=1")).toBe(false);
    expect(isAllowedEmbed("javascript:alert(1)")).toBe(false);
    expect(isAllowedEmbed("https://player.vimeo.com.evil.example/video/1")).toBe(false);
  });
});
