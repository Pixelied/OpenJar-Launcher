// Coalesces repeated actions into compact rows, then filters and buckets so dense activity remains easy to scan.
import { useMemo, type ReactNode } from "react";
import type {
  ActivityBucket,
  ActivityBucketLabel,
  RecentActivityFeedEntry,
  RecentActivityFilter,
} from "./types";

function bucketFor(atMs: number): ActivityBucketLabel {
  const now = Date.now();
  if (now - atMs < 5 * 60 * 1000) return "Just now";
  const nowDate = new Date(now);
  nowDate.setHours(0, 0, 0, 0);
  const entryDate = new Date(atMs);
  entryDate.setHours(0, 0, 0, 0);
  const dayDiff = Math.round((nowDate.getTime() - entryDate.getTime()) / 86_400_000);
  if (dayDiff <= 0) return "Today";
  if (dayDiff === 1) return "Yesterday";
  return "Earlier";
}

function normalizeTargetKey(target: string): string {
  return String(target ?? "")
    .trim()
    .toLowerCase()
    .replace(/\s+/g, " ");
}

function coalesce(entries: RecentActivityFeedEntry[], windowMs: number): RecentActivityFeedEntry[] {
  const out: RecentActivityFeedEntry[] = [];
  for (const entry of entries) {
    const prev = out[out.length - 1];
    const sameTarget = prev && normalizeTargetKey(prev.target) === normalizeTargetKey(entry.target);
    const sameMessage = prev && normalizeSummaryKey(prev.message) === normalizeSummaryKey(entry.message);
    const pinPair =
      prev &&
      prev.category === "pins" &&
      entry.category === "pins" &&
      sameTarget;
    const same =
      prev &&
      prev.category === entry.category &&
      (prev.rawKind === entry.rawKind || sameMessage || pinPair) &&
      Math.abs(prev.atMs - entry.atMs) <= windowMs;
    if (!same) {
      out.push(entry);
      continue;
    }
    const prevRaw = prev.rawEvents ?? [
      { id: prev.id, summary: prev.message, kind: prev.rawKind, atMs: prev.atMs },
    ];
    prev.rawEvents = [
      ...prevRaw,
      { id: entry.id, summary: entry.message, kind: entry.rawKind, atMs: entry.atMs },
    ];
    prev.coalescedCount = prev.rawEvents.length;
  }
  return out.map(finalizeCoalescedEntry);
}

function normalizeSummaryKey(summary: string): string {
  return String(summary ?? "")
    .trim()
    .toLowerCase()
    .replace(/\s+/g, " ");
}

function sanitizeSummary(summary: string): string {
  const trimmed = String(summary ?? "").trim().replace(/\s+/g, " ");
  if (!trimmed) return "Activity event";
  if (/github checks paused due to rate limit/i.test(trimmed)) {
    const skippedMatch = trimmed.match(/skipped\s+(\d+)\s+github\s+entries/i);
    const skipped = skippedMatch ? Number(skippedMatch[1]) : null;
    return skipped && Number.isFinite(skipped)
      ? `GitHub rate limit paused updates (${skipped} skipped)`
      : "GitHub rate limit paused updates";
  }
  return trimmed;
}

function truncateSentence(text: string, max = 92): string {
  const normalized = String(text ?? "").trim().replace(/\s+/g, " ");
  if (normalized.length <= max) return normalized;
  return `${normalized.slice(0, max - 1).trimEnd()}…`;
}

function normalizeEntityName(raw: string): string {
  const cleaned = String(raw ?? "").trim().replace(/\.(jar|zip|json|toml|yml|yaml|txt)$/i, "");
  const noVersionTail = cleaned.replace(/[-_.]?\d+(?:\.\d+)+(?:[-_.]?[a-z0-9]+)?$/i, "");
  return (noVersionTail || cleaned).replace(/[_-]+/g, " ").trim();
}

function extractEntityFromSummary(summary: string): string | null {
  const text = String(summary ?? "").trim();
  if (!text) return null;
  const singleQuoted = text.match(/'([^']+)'/);
  if (singleQuoted?.[1]) return normalizeEntityName(singleQuoted[1]);
  const doubleQuoted = text.match(/"([^"]+)"/);
  if (doubleQuoted?.[1]) return normalizeEntityName(doubleQuoted[1]);
  const fileLike = text.match(/([a-z0-9._-]+\.(?:jar|zip|json|toml|yml|yaml|txt))/i);
  if (fileLike?.[1]) return normalizeEntityName(fileLike[1]);
  const forTarget = text.match(/\bfor\s+([a-z0-9._:-]{3,})/i);
  if (forTarget?.[1]) return normalizeEntityName(forTarget[1]);
  return null;
}

function isGenericEntity(value: string): boolean {
  const normalized = value.toLowerCase();
  return (
    normalized.length < 3 ||
    normalized === "instance" ||
    normalized === "mods" ||
    normalized === "content" ||
    normalized === "entries" ||
    normalized === "local" ||
    normalized === "unknown" ||
    normalized.startsWith("update warning")
  );
}

function dedupeRawEvents(events: Array<{ id: string; summary: string; kind: string; atMs: number }>) {
  const deduped = new Map<
    string,
    { id: string; summary: string; kind: string; atMs: number; count: number }
  >();
  for (const event of events) {
    const key = `${String(event.kind ?? "").toLowerCase()}::${normalizeSummaryKey(event.summary)}`;
    const existing = deduped.get(key);
    if (!existing) {
      deduped.set(key, {
        id: event.id,
        summary: sanitizeSummary(event.summary),
        kind: event.kind,
        atMs: event.atMs,
        count: 1,
      });
      continue;
    }
    existing.count += 1;
    if (event.atMs > existing.atMs) {
      existing.atMs = event.atMs;
      existing.id = event.id;
      existing.summary = sanitizeSummary(event.summary);
    }
  }
  return Array.from(deduped.values())
    .sort((a, b) => b.atMs - a.atMs)
    .map((item) => ({
      id: item.id,
      summary: item.count > 1 ? `${item.summary} (x${item.count})` : item.summary,
      kind: item.kind,
      atMs: item.atMs,
    }));
}

function summarizeCoalescedMessage(entry: RecentActivityFeedEntry): string {
  const rawItems = entry.rawEvents ?? [
    { id: entry.id, summary: entry.message, kind: entry.rawKind, atMs: entry.atMs },
  ];
  const summaries = rawItems.map((item) => String(item.summary ?? ""));
  const lower = summaries.join(" ").toLowerCase();
  const hasRateLimit = /github checks paused due to rate limit|github.*rate limit/.test(lower);
  const hasUpdateAll =
    /\bupdating all\b/.test(lower) ||
    /\bupdated all\b/.test(lower) ||
    /\bupdated\s+\d+\s+entr(?:y|ies)\s+in\s+mods\b/.test(lower);
  if (entry.coalescedCount && entry.coalescedCount > 1) {
    if (entry.category === "updates") {
      if (hasRateLimit) return "Update paused by GitHub rate limit";
      if (hasUpdateAll) return "All mods updated";
      if ((entry.summaryTargetList?.length ?? 0) > 0) {
        const count = entry.summaryTargetList?.length ?? 0;
        return `Updated ${count} mod${count === 1 ? "" : "s"}`;
      }
      return `Updated ${entry.coalescedCount} items`;
    }
    if (entry.category === "warnings") {
      return hasRateLimit
        ? "Update warnings from GitHub rate limit"
        : `${entry.coalescedCount} warning event${entry.coalescedCount === 1 ? "" : "s"}`;
    }
    if (entry.category === "imports") return `Imported ${entry.coalescedCount} item${entry.coalescedCount === 1 ? "" : "s"}`;
    if (entry.category === "pins") return `${entry.coalescedCount} pin change${entry.coalescedCount === 1 ? "" : "s"}`;
    if (entry.category === "installs") return `${entry.coalescedCount} install/resolve event${entry.coalescedCount === 1 ? "" : "s"}`;
    return `${entry.coalescedCount} recent events`;
  }
  if (hasRateLimit) return "Update warning: GitHub rate limit";
  return truncateSentence(sanitizeSummary(entry.message));
}

function finalizeCoalescedEntry(entry: RecentActivityFeedEntry): RecentActivityFeedEntry {
  const rawItems = entry.rawEvents ?? [
    { id: entry.id, summary: entry.message, kind: entry.rawKind, atMs: entry.atMs },
  ];
  const dedupedRaw = dedupeRawEvents(rawItems);
  const summaryTargetList = Array.from(
    new Set(
      dedupedRaw
        .map((item) => extractEntityFromSummary(item.summary))
        .filter((value): value is string => Boolean(value && !isGenericEntity(value)))
    )
  ).slice(0, 8);
  const summarizedMessage = summarizeCoalescedMessage({
    ...entry,
    rawEvents: dedupedRaw,
    summaryTargetList,
  });
  const summarizedTarget =
    entry.category === "updates" && summaryTargetList.length > 0
      ? summaryTargetList.length > 3
        ? `${summaryTargetList.slice(0, 3).join(", ")} +${summaryTargetList.length - 3} more`
        : summaryTargetList.join(", ")
      : entry.target;
  return {
    ...entry,
    message: summarizedMessage,
    target: truncateSentence(summarizedTarget, 72),
    sourceLabel:
      entry.category === "pins"
        ? "Content Pin"
        : entry.sourceLabel,
    rawEvents: dedupedRaw,
    summaryTargetList,
  };
}

export interface ActivityCoalescerProps {
  entries: RecentActivityFeedEntry[];
  filter: RecentActivityFilter;
  limit: number;
  windowMs: number;
  showEarlierBucket: boolean;
  children: (payload: { entries: RecentActivityFeedEntry[]; grouped: ActivityBucket[] }) => ReactNode;
}

export default function ActivityCoalescer(props: ActivityCoalescerProps) {
  const payload = useMemo(() => {
    const sorted = [...props.entries].sort((a, b) => b.atMs - a.atMs);
    const merged = coalesce(sorted, props.windowMs).slice(0, props.limit);
    const filtered = props.filter === "all" ? merged : merged.filter((entry) => entry.category === props.filter);
    const labels: ActivityBucketLabel[] = props.showEarlierBucket
      ? ["Just now", "Today", "Yesterday", "Earlier"]
      : ["Just now", "Today", "Yesterday"];
    const grouped = labels
      .map((label) => ({
        label,
        items: filtered.filter((entry) => bucketFor(entry.atMs) === label),
      }))
      .filter((group) => group.items.length > 0);
    return { entries: filtered, grouped };
  }, [props.entries, props.filter, props.limit, props.windowMs, props.showEarlierBucket]);

  return <>{props.children(payload)}</>;
}
