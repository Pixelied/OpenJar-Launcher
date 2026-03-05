// Activity feed model types are centralized here so App.tsx can focus on orchestration/state instead of view detail.
import type { IconName } from "../app-shell/Icon";

export type RecentActivityFilter = "all" | "installs" | "updates" | "pins" | "imports" | "warnings";
export type ActivityBucketLabel = "Just now" | "Today" | "Yesterday" | "Earlier";

export type RecentActivityFeedEntry = {
  id: string;
  atMs: number;
  tone: "info" | "success" | "warn" | "error";
  message: string;
  target: string;
  sourceLabel: string;
  rawKind: string;
  category: RecentActivityFilter;
  icon: IconName;
  accent: "blue" | "purple" | "green" | "amber" | "neutral";
  exactTime: string;
  relativeTime: string;
  coalescedCount?: number;
  summaryTargetList?: string[];
  rawEvents?: Array<{ id: string; summary: string; kind: string; atMs: number }>;
};

export type ActivityBucket = {
  label: ActivityBucketLabel;
  items: RecentActivityFeedEntry[];
};
