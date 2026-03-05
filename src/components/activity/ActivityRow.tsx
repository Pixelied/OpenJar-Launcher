// Each row enforces clear hierarchy: action line, muted context line, and concise time with exact hover timestamp.
import { formatDateTime } from "../../app/utils/format";
import Icon from "../app-shell/Icon";
import type { RecentActivityFeedEntry } from "./types";

export interface ActivityRowProps {
  entry: RecentActivityFeedEntry;
  expanded: boolean;
  onToggleExpanded: (id: string) => void;
  showRawEvents?: boolean;
}

export default function ActivityRow(props: ActivityRowProps) {
  const { entry } = props;
  const canExpand = Boolean((entry.coalescedCount ?? 0) > 1 || entry.message.length > 90);
  return (
    <div className={`activityRow tone-${entry.tone} accent-${entry.accent}`}>
      <span className={`activityRowIcon accent-${entry.accent}`} aria-hidden="true">
        <Icon name={entry.icon} size={14} />
      </span>
      <div className="activityRowBody">
        <div className="activityRowPrimaryWrap">
          {canExpand ? (
            <button className="activityExpandBtn" type="button" onClick={() => props.onToggleExpanded(entry.id)}>
              <span className={`activityCaret ${props.expanded ? "expanded" : ""}`} aria-hidden="true">
                <Icon name="chevron_down" size={11} />
              </span>
              <span className={`activityRowPrimary ${props.expanded ? "expanded" : ""}`}>{entry.message}</span>
            </button>
          ) : (
            <div className="activityRowPrimary">{entry.message}</div>
          )}
          {entry.coalescedCount && entry.coalescedCount > 1 ? (
            <span className="chip subtle">x{entry.coalescedCount}</span>
          ) : null}
        </div>
        <div className="activityRowSecondary">{entry.sourceLabel} • {entry.target}</div>
        <time className="activityTime" data-oj-tooltip={entry.exactTime}>
          {entry.relativeTime}
        </time>
        {props.showRawEvents !== false && props.expanded && entry.rawEvents?.length ? (
          <div className="activityRawList">
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
