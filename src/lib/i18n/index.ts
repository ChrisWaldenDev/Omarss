// UI strings live in one locale file per language, keyed by id (SPEC §12). Only `en` for v1.

import en from "./en.json";

export type MessageKey = keyof typeof en;
export type MessageParams = Record<string, string | number>;

const messages: Record<MessageKey, string> = en;

/** Looks up a UI string and fills `{name}` placeholders. Numbers are formatted for the locale. */
export function t(key: MessageKey, params?: MessageParams, locale?: string): string {
  const template = messages[key];
  if (!params) return template;
  return template.replace(/\{(\w+)\}/g, (placeholder, name: string) => {
    const value = params[name];
    if (value === undefined) return placeholder;
    return typeof value === "number" ? value.toLocaleString(locale) : value;
  });
}
