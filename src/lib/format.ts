// Locale-aware formatting for dates, sizes and feed avatars. `locale` defaults to the user's.

const MINUTE = 60;
const HOUR = 60 * MINUTE;
const DAY = 24 * HOUR;

/** Short relative time for lists: "5 min. ago", "yesterday", or a date for older items. */
export function relativeTime(unixSeconds: number, nowMs: number, locale?: string): string {
  const diff = Math.round(nowMs / 1000) - unixSeconds;
  const rtf = new Intl.RelativeTimeFormat(locale, { numeric: "auto", style: "short" });
  if (diff < MINUTE) return rtf.format(0, "second");
  if (diff < HOUR) return rtf.format(-Math.floor(diff / MINUTE), "minute");
  if (diff < DAY) return rtf.format(-Math.floor(diff / HOUR), "hour");
  if (diff < 7 * DAY) return rtf.format(-Math.floor(diff / DAY), "day");

  const date = new Date(unixSeconds * 1000);
  const sameYear = date.getFullYear() === new Date(nowMs).getFullYear();
  return new Intl.DateTimeFormat(locale, {
    month: "short",
    day: "numeric",
    year: sameYear ? undefined : "numeric",
  }).format(date);
}

/** Full date and time, e.g. for the reader header. */
export function fullDate(unixSeconds: number, locale?: string): string {
  return new Intl.DateTimeFormat(locale, { dateStyle: "medium", timeStyle: "short" }).format(
    new Date(unixSeconds * 1000),
  );
}

/** File size such as "24 MB". */
export function fileSize(bytes: number, locale?: string): string {
  const units = ["byte", "kilobyte", "megabyte", "gigabyte"] as const;
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return new Intl.NumberFormat(locale, {
    style: "unit",
    unit: units[unit],
    // Short "byte" reads oddly ("0 byte"); spell bytes out.
    unitDisplay: unit === 0 ? "long" : "short",
    maximumFractionDigits: value < 10 && unit > 0 ? 1 : 0,
  }).format(value);
}

const AVATAR_COLORS = [
  "#2f6fde",
  "#0f7d63",
  "#b4461b",
  "#7b4fd6",
  "#c02d5b",
  "#4f7a12",
  "#9a6700",
  "#3d5a80",
];

/** Letter and colour for a feed without a favicon (SPEC §7.5 fallback). */
export function letterAvatar(title: string): { letter: string; color: string } {
  const letter = Array.from(title.trim())[0]?.toUpperCase() ?? "?";
  let hash = 0;
  for (const ch of title) hash = (hash * 31 + ch.codePointAt(0)!) >>> 0;
  return { letter, color: AVATAR_COLORS[hash % AVATAR_COLORS.length] };
}
