import { describe, expect, it } from "vitest";

import en from "./i18n/en.json";
import {
  ACTION_IDS,
  actionLabel,
  boundTo,
  conflictsWith,
  defaultBindings,
  eventToKey,
  isTypingTarget,
  keyParts,
  KeyDispatcher,
  overridesFrom,
  resolveBindings,
} from "./keyboard";

function key(k: string, mods: Partial<Record<"ctrl" | "alt" | "meta" | "shift", boolean>> = {}) {
  return {
    key: k,
    ctrlKey: !!mods.ctrl,
    altKey: !!mods.alt,
    metaKey: !!mods.meta,
    shiftKey: !!mods.shift,
  };
}

describe("eventToKey", () => {
  it("names keys the way bindings are written", () => {
    expect(eventToKey(key("j"))).toBe("j");
    expect(eventToKey(key("J", { shift: true }))).toBe("J");
    expect(eventToKey(key("?", { shift: true }))).toBe("?");
    expect(eventToKey(key(" "))).toBe("Space");
    expect(eventToKey(key(" ", { shift: true }))).toBe("Shift+Space");
    expect(eventToKey(key(",", { ctrl: true }))).toBe("Ctrl+,");
    expect(eventToKey(key("Enter"))).toBe("Enter");
  });

  it("ignores lone modifiers", () => {
    for (const k of ["Shift", "Control", "Alt", "Meta", "Dead"]) {
      expect(eventToKey(key(k))).toBeNull();
    }
  });
});

describe("KeyDispatcher", () => {
  it("maps single keys to the spec's default actions", () => {
    const dispatcher = new KeyDispatcher(defaultBindings);
    expect(dispatcher.handle("j")).toBe("nextArticle");
    expect(dispatcher.handle("J")).toBe("nextUnreadFeed");
    expect(dispatcher.handle("Enter")).toBe("openSelected");
    expect(dispatcher.handle("o")).toBe("openSelected");
    expect(dispatcher.handle("A")).toBe("markAllRead");
    expect(dispatcher.handle("Ctrl+,")).toBe("openSettings");
    expect(dispatcher.handle("x")).toBeNull();
  });

  it("handles two-key sequences with a timeout", () => {
    let now = 0;
    const dispatcher = new KeyDispatcher(defaultBindings, 1000, () => now);
    expect(dispatcher.handle("g")).toBe("pending");
    expect(dispatcher.handle("u")).toBe("goUnread");

    expect(dispatcher.handle("g")).toBe("pending");
    now += 2000;
    expect(dispatcher.handle("s")).toBe("toggleStar"); // too late: just "s"

    expect(dispatcher.handle("g")).toBe("pending");
    expect(dispatcher.handle("j")).toBe("nextArticle"); // not a sequence: the key alone
  });

  it("uses rebound keys", () => {
    const bindings = resolveBindings({ nextArticle: ["ArrowDown"], bogus: ["x"] });
    const dispatcher = new KeyDispatcher(() => bindings);
    expect(dispatcher.handle("ArrowDown")).toBe("nextArticle");
    expect(dispatcher.handle("j")).toBeNull();
    expect(dispatcher.handle("x")).toBeNull();
  });
});

describe("bindings", () => {
  it("saves only what differs from the defaults", () => {
    const bindings = defaultBindings();
    expect(overridesFrom(bindings)).toEqual({});
    bindings.toggleStar = ["f"];
    expect(overridesFrom(bindings)).toEqual({ toggleStar: ["f"] });
    expect(resolveBindings(overridesFrom(bindings)).toggleStar).toEqual(["f"]);
  });

  it("finds which action already uses a key", () => {
    const bindings = defaultBindings();
    expect(boundTo(bindings, "s")).toBe("toggleStar");
    expect(boundTo(bindings, "s", "toggleStar")).toBeNull();
    expect(boundTo(bindings, "g a")).toBe("goAll");
  });

  it("has no conflicting defaults and a label for every action", () => {
    const seen = new Set<string>();
    for (const id of ACTION_IDS) {
      expect(Object.keys(en)).toContain(actionLabel(id));
      for (const binding of defaultBindings()[id]) {
        expect(seen.has(binding)).toBe(false);
        seen.add(binding);
      }
    }
  });

  it("detects bindings that clash or shadow a sequence", () => {
    const bindings = defaultBindings();
    expect(conflictsWith(bindings, "s", "toggleRead")).toBe("toggleStar");
    expect(conflictsWith(bindings, "g", "toggleRead")).toBe("goAll");
    expect(conflictsWith(bindings, "j x", "toggleRead")).toBe("nextArticle");
    expect(conflictsWith(bindings, "g x", "toggleRead")).toBeNull();
    expect(conflictsWith(bindings, "x", "toggleRead")).toBeNull();
    expect(conflictsWith(bindings, "m", "toggleRead")).toBeNull();
  });

  it("splits bindings for display", () => {
    expect(keyParts("Shift+Space")).toEqual([["Shift", "Space"]]);
    expect(keyParts("g a")).toEqual([["g"], ["a"]]);
    expect(keyParts("Ctrl++")).toEqual([["Ctrl", "+"]]);
    expect(keyParts("Ctrl+,")).toEqual([["Ctrl", ","]]);
  });
});

describe("isTypingTarget", () => {
  const el = (tagName: string, extra: Record<string, unknown> = {}) =>
    ({ tagName, ...extra }) as unknown as EventTarget;

  it("treats text fields as typing, other controls as not", () => {
    expect(isTypingTarget(el("INPUT", { type: "text" }))).toBe(true);
    expect(isTypingTarget(el("INPUT", { type: "search" }))).toBe(true);
    expect(isTypingTarget(el("TEXTAREA"))).toBe(true);
    expect(isTypingTarget(el("SELECT"))).toBe(true);
    expect(isTypingTarget(el("DIV", { isContentEditable: true }))).toBe(true);
    expect(isTypingTarget(el("INPUT", { type: "checkbox" }))).toBe(false);
    expect(isTypingTarget(el("INPUT", { type: "range" }))).toBe(false);
    expect(isTypingTarget(el("BUTTON"))).toBe(false);
    expect(isTypingTarget(el("DIV", { isContentEditable: false }))).toBe(false);
    expect(isTypingTarget(null)).toBe(false);
  });
});
