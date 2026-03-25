import ActivityFilters from "./ActivityFilters";
import type { RecentActivityFilter } from "./types";

// Sidebar header keeps retention/context visible while keeping controls quiet so the log remains the visual priority.
export interface ActivitySidebarHeaderProps {
  retentionLabel: string;
  filter: RecentActivityFilter;
  onFilterChange: (value: RecentActivityFilter) => void;
  onOpenFullHistory: () => void;
  onClearRecent: () => void;
  canClear: boolean;
}

export default function ActivitySidebarHeader(props: ActivitySidebarHeaderProps) {
  return (
    <div className="activityHead">
      <div className="activityHeadTop">
        <div className="activityHeadIntro">
          <div className="librarySideTitle activityHeaderTitle">Timeline</div>
        </div>
        <div className="activityActions">
          <button
            className="btn ghost activityActionBtn activityHistoryBtn"
            onClick={props.onOpenFullHistory}
            data-oj-tooltip="Open the complete event log with pagination."
          >
            History
          </button>
          <button
            className="btn ghost activityActionBtn activityAuxAction"
            onClick={props.onClearRecent}
            disabled={!props.canClear}
            data-oj-tooltip="Clear only this recent timeline view."
          >
            Clear
          </button>
        </div>
      </div>
      <div className="activityHeadMeta">
        <div className="activityHeadMetaCopy">
          <div className="activityMetaLabel">Recent view</div>
          <div className="activitySub">{props.retentionLabel}</div>
        </div>
        <div className="activityHeadMetaFilter">
          <ActivityFilters value={props.filter} onChange={props.onFilterChange} compact />
        </div>
      </div>
    </div>
  );
}
