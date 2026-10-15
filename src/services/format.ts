const relativeTime = new Intl.RelativeTimeFormat(undefined, { numeric: "auto" });
const clockTime = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit" });
const clockTimeWithSeconds = new Intl.DateTimeFormat(undefined, { hour: "numeric", minute: "2-digit", second: "2-digit" });
const dateTime = new Intl.DateTimeFormat(undefined, { dateStyle: "medium", timeStyle: "short" });

const RELATIVE_STEPS: Array<[Intl.RelativeTimeFormatUnit, number]> = [
  ["second", 60],
  ["minute", 60],
  ["hour", 24],
  ["day", 7],
  ["week", 4.35],
  ["month", 12],
  ["year", Number.POSITIVE_INFINITY],
];

/** "3 minutes ago", "in 2 hours". */
export function formatRelative(iso: string, now: number = Date.now()): string {
  let value = (new Date(iso).getTime() - now) / 1000;
  for (const [unit, size] of RELATIVE_STEPS) {
    if (Math.abs(value) < size) {
      if (unit === "second" && Math.abs(value) < 45) return value > 0 ? "in a moment" : "just now";
      return relativeTime.format(Math.round(value), unit);
    }
    value /= size;
  }
  return relativeTime.format(Math.round(value), "year");
}

/** Rotations faster than this show the next change to the second. */
const PRECISE_INTERVAL = 10 * 60;

/**
 * "at 2:05 PM" today, "on Oct 7, 2026, 9:00 AM" on other days. With an
 * interval under ten minutes, today's time includes seconds: "at 2:05:30 PM".
 */
export function describeNextChange(iso: string, now: number = Date.now(), intervalSeconds?: number | null): string {
  const date = new Date(Math.max(new Date(iso).getTime(), now));
  const sameDay = new Date(now).toDateString() === date.toDateString();
  if (!sameDay) return `on ${dateTime.format(date)}`;
  const precise = intervalSeconds != null && intervalSeconds < PRECISE_INTERVAL;
  return `at ${(precise ? clockTimeWithSeconds : clockTime).format(date)}`;
}

export function formatDateTime(iso: string): string {
  return dateTime.format(new Date(iso));
}

export function formatBytes(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let value = bytes / 1024;
  let unit = 0;
  while (value >= 1024 && unit < units.length - 1) {
    value /= 1024;
    unit += 1;
  }
  return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} ${units[unit]}`;
}

export function formatResolution(width: number, height: number): string | null {
  return width > 0 && height > 0 ? `${width} × ${height}` : null;
}

export function pluralize(count: number, singular: string, plural = `${singular}s`): string {
  return `${count} ${count === 1 ? singular : plural}`;
}

/** Shortest and longest rotation intervals the backend accepts, in seconds. */
export const MIN_INTERVAL_SECONDS = 5;
export const MAX_INTERVAL_SECONDS = 86_400;

/** Preset rotation intervals, in seconds. */
export const INTERVAL_OPTIONS = [
  { seconds: 60, label: "1 min" },
  { seconds: 300, label: "5 min" },
  { seconds: 900, label: "15 min" },
  { seconds: 1800, label: "30 min" },
  { seconds: 3600, label: "1 hour" },
  { seconds: 86_400, label: "Daily" },
] as const;

/** "every 45 seconds", "every minute", "every 2 minutes 30 seconds", "once a day". */
export function describeInterval(seconds: number): string {
  if (seconds === 86_400) return "once a day";
  if (seconds === 3600) return "every hour";
  if (seconds === 60) return "every minute";
  const parts = [
    [Math.floor(seconds / 3600), "hour"],
    [Math.floor((seconds % 3600) / 60), "minute"],
    [seconds % 60, "second"],
  ] as const;
  const words = parts.filter(([count]) => count > 0).map(([count, unit]) => pluralize(count, unit));
  return `every ${words.join(" ")}`;
}
