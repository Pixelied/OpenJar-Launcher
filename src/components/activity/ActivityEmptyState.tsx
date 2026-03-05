// Compact empty state keeps the panel informative without dominating layout and clarifies retention behavior.
import Icon from "../app-shell/Icon";

export interface ActivityEmptyStateProps {
  loading: boolean;
}

export default function ActivityEmptyState(props: ActivityEmptyStateProps) {
  return (
    <div className="compactEmptyState activityEmptyState">
      <span className="compactEmptyIcon" aria-hidden="true">
        <Icon name="sparkles" size={14} />
      </span>
      <div className="compactEmptyBody">
        <div className="compactEmptyTitle">{props.loading ? "Loading timeline…" : "No recent events"}</div>
        <div className="compactEmptyText">Recent-only feed. Open Full history for older events.</div>
      </div>
    </div>
  );
}
