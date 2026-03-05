// Sidebar rows are flattened into log-style entries with icon-only type color and clear 3-line text hierarchy.
import { formatDateTime } from "../../app/utils/format";
import Icon from "../app-shell/Icon";
import type { RecentActivityFeedEntry } from "./types";

export interface ActivitySidebarRowProps {
  entry: RecentActivityFeedEntry;
  expanded: boolean;
  onToggleExpanded: (id: string) => void;
  showRawEvents?: boolean;
}

export default function ActivitySidebarRow(props: ActivitySidebarRowProps) {
  const { entry } = props;
  const canExpand = Boolean(
    (entry.rawEvents?.length ?? 0) > 0 ||
      (entry.coalescedCount ?? 0) > 1 ||
      entry.message.length > 56 ||
      entry.target.length > 44
  );
  const hasSummaryTargets = Array.isArray(entry.summaryTargetList) && entry.summaryTargetList.length > 0;
  return (
    <div className={`activityRow tone-${entry.tone} accent-${entry.accent}`} role="listitem">
      <span className={`activityRowIcon accent-${entry.accent}`} aria-hidden="true">
        <Icon name={entry.icon} size={14} />
      </span>
      <div className="activityRowBody">
        <div className="activityRowPrimaryWrap">
          {canExpand ? (
            <button
              className={`activityExpandBtn ${props.expanded ? "expanded" : ""}`}
              type="button"
              onClick={() => props.onToggleExpanded(entry.id)}
              aria-expanded={props.expanded}
            >
              <span className={`activityCaret ${props.expanded ? "expanded" : ""}`} aria-hidden="true">
                <Icon name="chevron_down" size={12} />
              </span>
              <span className={`activityRowPrimary ${props.expanded ? "expanded" : ""}`} title={entry.message}>
                {entry.message}
              </span>
            </button>
          ) : (
            <div className="activityRowPrimary" title={entry.message}>
              {entry.message}
            </div>
          )}
          {entry.coalescedCount && entry.coalescedCount > 1 ? (
            <span className="activityRowCount" aria-label={`${entry.coalescedCount} coalesced events`}>
              x{entry.coalescedCount}
            </span>
          ) : null}
        </div>
        <div className={`activityRowSecondary ${props.expanded ? "expanded" : ""}`} title={entry.target}>
          {entry.target}
        </div>
        <time className="activityTime" data-oj-tooltip={entry.exactTime}>
          {entry.sourceLabel} • {entry.relativeTime}
        </time>
        {props.showRawEvents !== false && props.expanded && entry.rawEvents?.length ? (
          <div className="activityRawList">
            {entry.category === "updates" && hasSummaryTargets ? (
              <div className="activityRawHint">
                Updated mods: {entry.summaryTargetList!.slice(0, 6).join(", ")}
                {entry.summaryTargetList!.length > 6 ? ` +${entry.summaryTargetList!.length - 6} more` : ""}
              </div>
            ) : null}
            {entry.rawEvents.map((raw) => (
              <div key={raw.id} className="activityRawItem">
                {formatDateTime(new Date(raw.atMs).toISOString(), "Unknown")} • {raw.summary}
              </div>
            ))}
          </div>
        ) : null}
      </div>
    </div>
  );
}
