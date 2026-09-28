// Test fixture: the backend's default settings (see `Settings::default` in Rust).

import type { Settings } from "../types";

export function defaultSettings(overrides: Partial<Settings> = {}): Settings {
  return {
    theme: "system",
    refreshIntervalMinutes: 30,
    refreshOnStartup: true,
    markUpdatedUnread: false,
    pauseOnMetered: false,
    layout: "threePane",
    markReadMode: "onOpen",
    markReadDelaySeconds: 3,
    listThumbnails: true,
    readerFont: "system",
    readerFontSize: 17,
    readerLineWidth: 70,
    readerLineHeight: 165,
    accentColor: null,
    uiScale: 100,
    sidebarWidth: 240,
    listWidth: 360,
    retentionDays: 90,
    loadImages: "always",
    stripTrackingParams: true,
    imageCacheMb: 500,
    proxyUrl: "",
    shortcuts: {},
    ...overrides,
  };
}
