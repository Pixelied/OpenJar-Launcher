import { useMemo, useState } from "react";
import ActivityCoalescer from "./ActivityCoalescer";
import ActivityEmptyState from "./ActivityEmptyState";
import ActivitySidebarHeader from "./ActivitySidebarHeader";
import ActivitySidebarList from "./ActivitySidebarList";
import type { RecentActivityFeedEntry, RecentActivityFilter } from "./types";

export interface ActivityFeedProps {
  entriesRaw: RecentActivityFeedEntry[];
  loading: boolean;
  filter: RecentActivityFilter;
  onFilterChange: (value: RecentActivityFilter) => void;
  retentionLabel: string;
  onOpenFullHistory: () => void;
  onClearRecent: () => void;
  canClear: boolean;
  windowMs: number;
  limit: number;
  showEarlierBucket: boolean;
}

export default function ActivityFeed(props: ActivityFeedProps) {
  const [expandedById, setExpandedById] = useState<Record<string, boolean>>({});
  const hasAny = useMemo(() => props.entriesRaw.length > 0, [props.entriesRaw.length]);

  return (
    <div className="card instanceSideCard activityFeedCard">
      <ActivitySidebarHeader
        retentionLabel={props.retentionLabel}
        filter={props.filter}
        onFilterChange={props.onFilterChange}
        onOpenFullHistory={props.onOpenFullHistory}
        onClearRecent={props.onClearRecent}
        canClear={props.canClear}
      />

      {!hasAny ? (
        <ActivityEmptyState loading={props.loading} />
      ) : (
        <ActivityCoalescer
          entries={props.entriesRaw}
          filter={props.filter}
          limit={props.limit}
          windowMs={props.windowMs}
          showEarlierBucket={props.showEarlierBucket}
        >
          {({ entries, grouped }) =>
            entries.length === 0 ? (
              <ActivityEmptyState loading={props.loading} />
            ) : (
              <ActivitySidebarList
                grouped={grouped}
                expandedById={expandedById}
                onToggleExpanded={(id) =>
                  setExpandedById((prev) => ({
                    ...prev,
                    [id]: !prev[id],
                  }))
                }
              />
            )
          }
        </ActivityCoalescer>
      )}
    </div>
  );
}
