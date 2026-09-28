// Keyboard shortcuts (SPEC §6.7): default bindings, user overrides, turning key events into
// binding strings, and two-key sequences like "g a".
//
// A binding is a key as `KeyboardEvent.key` reports it ("j", "J", "?", "Enter"), with
// modifiers in front ("Ctrl+,", "Shift+Space"). Shift is only written for named keys: "J"
// already means Shift+j. A sequence is two bindings separated by a space ("g a").

import type { MessageKey } from "./i18n";

export const ACTIONS = [
  { id: "nextArticle", keys: ["j"] },
  { id: "prevArticle", keys: ["k"] },
  { id: "nextItem", keys: ["n"] },
  { id: "prevItem", keys: ["p"] },
  { id: "nextUnreadFeed", keys: ["J"] },
  { id: "prevUnreadFeed", keys: ["K"] },
  { id: "openSelected", keys: ["o", "Enter"] },
  { id: "openInBrowser", keys: ["v"] },
  { id: "toggleRead", keys: ["m"] },
  { id: "toggleStar", keys: ["s"] },
  { id: "markAllRead", keys: ["A"] },
  { id: "refreshCurrent", keys: ["r"] },
  { id: "refreshAll", keys: ["R"] },
  { id: "goAll", keys: ["g a"] },
  { id: "goUnread", keys: ["g u"] },
  { id: "goStarred", keys: ["g s"] },
  { id: "scrollDown", keys: ["Space"] },
  { id: "scrollUp", keys: ["Shift+Space"] },
  { id: "openSettings", keys: ["Ctrl+,"] },
  { id: "showShortcuts", keys: ["?"] },
] as const;

export type ActionId = (typeof ACTIONS)[number]["id"];
export type Bindings = Record<ActionId, string[]>;

export const ACTION_IDS: ActionId[] = ACTIONS.map((a) => a.id);

export function actionLabel(id: ActionId): MessageKey {
  return `shortcut.${id}` as MessageKey;
}

export function isActionId(id: string): id is ActionId {
  return (ACTION_IDS as string[]).includes(id);
}

export function defaultBindings(): Bindings {
  return Object.fromEntries(ACTIONS.map((a) => [a.id, [...a.keys]])) as Bindings;
}

/** Defaults with the user's overrides applied; unknown actions in `overrides` are ignored. */
export function resolveBindings(overrides: Partial<Record<string, string[]>>): Bindings {
  const bindings = defaultBindings();
  for (const [id, keys] of Object.entries(overrides)) {
    if (keys && isActionId(id)) bindings[id] = [...keys];
  }
  return bindings;
}

/** Only the bindings that differ from the defaults, for saving. */
export function overridesFrom(bindings: Bindings): Record<string, string[]> {
  const defaults = defaultBindings();
  const overrides: Record<string, string[]> = {};
  for (const id of ACTION_IDS) {
    if (bindings[id].join("\n") !== defaults[id].join("\n")) overrides[id] = bindings[id];
  }
  return overrides;
}

/** The action a key is bound to, other than `except`. */
export function boundTo(bindings: Bindings, key: string, except?: ActionId): ActionId | null {
  for (const id of ACTION_IDS) {
    if (id !== except && bindings[id].includes(key)) return id;
  }
  return null;
}

/**
 * The action a new binding would clash with: one using the same keys, or one it would make
 * unreachable (a single key that starts another action's sequence, or the reverse).
 */
export function conflictsWith(
  bindings: Bindings,
  binding: string,
  except: ActionId,
): ActionId | null {
  const first = binding.split(" ")[0];
  const isSequence = binding.includes(" ");
  for (const id of ACTION_IDS) {
    if (id === except) continue;
    for (const other of bindings[id]) {
      if (other === binding) return id;
      if (!isSequence && other.startsWith(`${binding} `)) return id;
      if (isSequence && other === first) return id;
    }
  }
  return null;
}

const MODIFIER_KEYS = new Set(["Shift", "Control", "Alt", "Meta", "AltGraph", "CapsLock"]);

type KeyEventLike = Pick<KeyboardEvent, "key" | "ctrlKey" | "altKey" | "metaKey" | "shiftKey">;

/** The binding string for a key press, or `null` for a lone modifier. */
export function eventToKey(event: KeyEventLike): string | null {
  if (MODIFIER_KEYS.has(event.key) || event.key === "Dead" || event.key === "Unidentified") {
    return null;
  }
  const key = event.key === " " ? "Space" : event.key;
  const modifiers: string[] = [];
  if (event.ctrlKey) modifiers.push("Ctrl");
  if (event.altKey) modifiers.push("Alt");
  if (event.metaKey) modifiers.push("Meta");
  // For characters, Shift is already part of the key ("J", "?").
  if (event.shiftKey && key.length > 1) modifiers.push("Shift");
  return [...modifiers, key].join("+");
}

/** How a binding is shown: "Shift+Space" → "Shift + Space", "g a" → "g then a". */
export function keyParts(binding: string): string[][] {
  return binding.split(" ").map((step) => (step === "+" ? ["+"] : step.split(/\+(?!$)/)));
}

const NON_TEXT_INPUTS = ["checkbox", "radio", "button", "submit", "reset", "range", "color"];

/** Whether keys typed here are text, not shortcuts (SPEC §6.7). */
export function isTypingTarget(target: EventTarget | null): boolean {
  const element = target as { tagName?: unknown; type?: unknown; isContentEditable?: unknown };
  if (!element || typeof element.tagName !== "string") return false;
  if (element.isContentEditable === true) return true;
  switch (element.tagName.toUpperCase()) {
    case "TEXTAREA":
    case "SELECT":
      return true;
    case "INPUT":
      return !NON_TEXT_INPUTS.includes(String(element.type ?? "text").toLowerCase());
    default:
      return false;
  }
}

/**
 * Turns key presses into actions, including two-key sequences: the first key of a sequence
 * waits up to `timeoutMs` for the second.
 */
export class KeyDispatcher {
  #pending: string | null = null;
  #pendingAt = 0;

  constructor(
    private readonly bindings: () => Bindings,
    private readonly timeoutMs = 1500,
    private readonly now: () => number = () => Date.now(),
  ) {}

  /** The action for `key`, `"pending"` if it starts a sequence, or `null`. */
  handle(key: string): ActionId | "pending" | null {
    const bindings = this.bindings();
    const pending = this.#pending;
    this.#pending = null;
    if (pending !== null && this.now() - this.#pendingAt <= this.timeoutMs) {
      const action = boundTo(bindings, `${pending} ${key}`);
      if (action) return action;
    }
    const action = boundTo(bindings, key);
    if (action) return action;
    const startsSequence = ACTION_IDS.some((id) =>
      bindings[id].some((binding) => binding.startsWith(`${key} `)),
    );
    if (startsSequence) {
      this.#pending = key;
      this.#pendingAt = this.now();
      return "pending";
    }
    return null;
  }

  reset(): void {
    this.#pending = null;
  }
}
