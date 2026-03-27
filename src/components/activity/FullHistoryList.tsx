import { useState } from "react";
import Icon from "../app-shell/Icon";
import ActivityCoalescer from "./ActivityCoalescer";
import ActivitySidebarList from "./ActivitySidebarList";
import type { RecentActivityFilter } from "./types";
import type { RecentActivityFeedEntry } from "./types";

export interface FullHistoryListProps {
  rows: RecentActivityFeedEntry[];
  busy: boolean;
  filter: RecentActivityFilter;
  coalesceWindowMs: number;
}

export default function FullHistoryList(props: FullHistoryListProps) {
  const [expandedById, setExpandedById] = useState<Record<string, boolean>>({});

  if (props.rows.length === 0) {
    return (
      <div className="instanceFullHistoryList">
        <div className="compactEmptyState fullHistoryEmptyState">
          <span className="compactEmptyIcon" aria-hidden="true">
            <Icon name="sparkles" size={14} />
          </span>
          <div className="compactEmptyBody">
            <div className="compactEmptyTitle">{props.busy ? "Loading full history…" : "No events found"}</div>
            <div className="compactEmptyText">Try a different filter or search term.</div>
          </div>
        </div>
      </div>
    );
  }

  return (
    <div className="instanceFullHistoryList fullHistoryTimelineLike" role="region" aria-live="polite">
      <ActivityCoalescer
        entries={props.rows}
        filter={props.filter}
        limit={Math.max(props.rows.length, 1)}
        windowMs={props.coalesceWindowMs}
        showEarlierBucket
      >
        {({ entries, grouped }) =>
          entries.length === 0 ? (
            <div className="compactEmptyState fullHistoryEmptyState">
              <span className="compactEmptyIcon" aria-hidden="true">
                <Icon name="sparkles" size={14} />
              </span>
              <div className="compactEmptyBody">
                <div className="compactEmptyTitle">No events match this filter</div>
                <div className="compactEmptyText">Try All or remove search terms.</div>
              </div>
            </div>
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
    </div>
  );
}
