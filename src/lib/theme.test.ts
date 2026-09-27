import { describe, expect, it } from "vitest";

import { applyTheme } from "./theme";

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
