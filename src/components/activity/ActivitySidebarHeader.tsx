// Sidebar header keeps retention/context visible while keeping controls quiet so the log remains the visual priority.
export interface ActivitySidebarHeaderProps {
  retentionLabel: string;
  onOpenFullHistory: () => void;
  onClearRecent: () => void;
  canClear: boolean;
}

export default function ActivitySidebarHeader(props: ActivitySidebarHeaderProps) {
  return (
    <div className="activityHead">
      <div className="activityHeadTop">
        <div className="librarySideTitle activityHeaderTitle">Timeline</div>
        <div className="activityActions">
          <button
            className="btn ghost activityActionBtn"
            onClick={props.onOpenFullHistory}
            data-oj-tooltip="Open the complete event log with pagination."
          >
            Full history
          </button>
          <button
            className="btn ghost activityActionBtn activityAuxAction"
            onClick={props.onClearRecent}
            disabled={!props.canClear}
            data-oj-tooltip="Clear only this recent timeline view."
          >
            Clear recent
          </button>
        </div>
      </div>
      <div className="activitySub">{props.retentionLabel}</div>
    </div>
  );
}
