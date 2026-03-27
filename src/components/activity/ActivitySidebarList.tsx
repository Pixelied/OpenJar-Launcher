import type { ActivityBucket } from "./types";
import ActivityGroupHeader from "./ActivityGroupHeader";
import ActivitySidebarRow from "./ActivitySidebarRow";

export interface ActivitySidebarListProps {
  grouped: ActivityBucket[];
  expandedById: Record<string, boolean>;
  onToggleExpanded: (id: string) => void;
}

export default function ActivitySidebarList(props: ActivitySidebarListProps) {
  return (
    <div className="activityList">
      {props.grouped.map((group) => (
        <section key={group.label} className="activityGroup">
          <ActivityGroupHeader label={group.label} />
          <div className="activityGroupRows" role="list">
            {group.items.map((entry) => (
              <ActivitySidebarRow
                key={entry.id}
                entry={entry}
                expanded={Boolean(props.expandedById[entry.id])}
                onToggleExpanded={props.onToggleExpanded}
              />
            ))}
          </div>
        </section>
      ))}
    </div>
  );
}
