import { describe, expect, it } from "vitest";

import { defaultSettings } from "./testing/settings";
import { applyAppearance, applyTheme, contrastText, READER_FONTS } from "./theme";

describe("applyTheme", () => {
  it("sets an explicit theme and clears it for system", () => {
    const root = { dataset: {} as DOMStringMap };
    applyTheme("dark", root);
    expect(root.dataset.theme).toBe("dark");
    applyTheme("light", root);
    expect(root.dataset.theme).toBe("light");
    applyTheme("system", root);
    expect(root.dataset.theme).toBeUndefined();
  });
});

describe("applyAppearance", () => {
  function style() {
    const props = new Map<string, string>();
    return {
      props,
      setProperty: (name: string, value: string) => void props.set(name, value),
      removeProperty: (name: string) => {
        const old = props.get(name) ?? "";
        props.delete(name);
        return old;
      },
    };
  }

  it("sets reader typography", () => {
    const target = style();
    applyAppearance(
      defaultSettings({
        readerFont: "serif",
        readerFontSize: 20,
        readerLineHeight: 180,
        readerLineWidth: 60,
      }),
      target,
    );
    expect(target.props.get("--reader-font")).toBe(READER_FONTS.serif);
    expect(target.props.get("--reader-font-size")).toBe("20px");
    expect(target.props.get("--reader-line-height")).toBe("1.8");
    expect(target.props.get("--reader-width")).toBe("60ch");
  });

  it("overrides the accent colour and restores the theme's", () => {
    const target = style();
    applyAppearance(defaultSettings({ accentColor: "#ffd400" }), target);
    expect(target.props.get("--accent")).toBe("#ffd400");
    expect(target.props.get("--accent-contrast")).toBe("#000000");
    applyAppearance(defaultSettings(), target);
    expect(target.props.has("--accent")).toBe(false);
    expect(target.props.has("--bg-selected")).toBe(false);
  });

  it("picks readable text for an accent", () => {
    expect(contrastText("#1f5bc4")).toBe("#ffffff");
    expect(contrastText("#ffffff")).toBe("#000000");
    expect(contrastText("#000000")).toBe("#ffffff");
  });
});
