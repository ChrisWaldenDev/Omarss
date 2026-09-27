import { convertFileSrc } from "@tauri-apps/api/core";

/** URL of a cached favicon, served by the backend's `omarss-img` protocol (SPEC §7.5). */
export function feedIconUrl(icon: string): string {
  return convertFileSrc(`icon/${icon}`, "omarss-img");
}
