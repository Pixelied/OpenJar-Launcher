export function formatCompact(n: number) {
  if (!Number.isFinite(n)) return String(n);
  if (n >= 1_000_000_000) return `${(n / 1_000_000_000).toFixed(1)}B`;
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return String(n);
}

export function formatPercent(n: number | null | undefined) {
  if (n === null || n === undefined || !Number.isFinite(n)) return "";
  const clamped = Math.max(0, Math.min(100, n));
  if (clamped > 0 && clamped < 0.1) return "<0.1%";
  if (clamped < 10) return `${clamped.toFixed(1)}%`;
  return `${clamped.toFixed(0)}%`;
}

export function formatBytes(value: number | null | undefined) {
  const bytes = Number(value ?? 0);
  if (!Number.isFinite(bytes) || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let size = bytes;
  let index = 0;
  while (size >= 1024 && index < units.length - 1) {
    size /= 1024;
    index += 1;
  }
  const decimals = size >= 100 || index === 0 ? 0 : 1;
  return `${size.toFixed(decimals)} ${units[index]}`;
}

export function formatDurationMs(ms: number | null | undefined) {
  if (ms == null || !Number.isFinite(ms) || ms < 0) return "";
  if (ms < 1000) return `${Math.round(ms)}ms`;
  const seconds = ms / 1000;
  if (seconds < 60) return `${seconds.toFixed(seconds < 10 ? 1 : 0)}s`;
  const minutes = Math.floor(seconds / 60);
  const remain = Math.round(seconds % 60);
  return `${minutes}m ${remain}s`;
}

export function formatEtaSeconds(seconds: number | null | undefined) {
  if (seconds == null || !Number.isFinite(seconds) || seconds < 0) return "Estimating…";
  if (seconds < 1) return "<1s";
  if (seconds < 60) return `${Math.round(seconds)}s`;
  const mins = Math.floor(seconds / 60);
  const sec = Math.round(seconds % 60);
  return `${mins}m ${sec}s`;
}

export function formatPerfActionLabel(name: string) {
  const parts = String(name)
    .split("_")
    .filter((part) => part.trim().length > 0);
  if (parts.length === 0) return "Action";
  return parts
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

export function parseDateLike(input: string | null | undefined): Date | null {
  if (!input) return null;
  const raw = String(input).trim();
  if (!raw) return null;
  if (raw.startsWith("unix:")) {
    const secs = Number(raw.slice(5).trim());
    if (Number.isFinite(secs) && secs > 0) {
      const fromUnix = new Date(secs * 1000);
      if (Number.isFinite(fromUnix.getTime())) return fromUnix;
    }
  }
  const d = new Date(raw);
  if (!Number.isFinite(d.getTime())) return null;
  return d;
}

export function formatDate(input: string | null | undefined) {
  const d = parseDateLike(input);
  if (!d) return input ?? "";
  return d.toLocaleDateString(undefined, {
    year: "numeric",
    month: "short",
    day: "numeric",
  });
}

export function formatDateTime(input: string | null | undefined, fallback = "Unknown date") {
  const d = parseDateLike(input);
  if (!d) return fallback;
  return d.toLocaleString();
}

export function formatFileSize(bytes: number | null | undefined) {
  if (!bytes || !Number.isFinite(bytes) || bytes <= 0) return "Unknown size";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let idx = 0;
  while (value >= 1024 && idx < units.length - 1) {
    value /= 1024;
    idx += 1;
  }
  const digits = value >= 10 || idx === 0 ? 0 : 1;
  return `${value.toFixed(digits)} ${units[idx]}`;
}

export function humanizeToken(value: string | null | undefined) {
  if (!value) return "Unknown";
  return value
    .replace(/[_-]+/g, " ")
    .replace(/\b\w/g, (m) => m.toUpperCase());
}
