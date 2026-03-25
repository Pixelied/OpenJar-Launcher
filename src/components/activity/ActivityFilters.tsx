// Activity filter chips are routed through the shared FilterChips control to keep emphasis subtle and consistent.
import MenuSelect from "../app-shell/controls/MenuSelect";
import FilterChips from "./FilterChips";
import type { RecentActivityFilter } from "./types";

const OPTIONS: Array<{ id: RecentActivityFilter; label: string; tone?: "default" | "warning" }> = [
  { id: "all", label: "All" },
  { id: "installs", label: "Installs" },
  { id: "updates", label: "Updates" },
  { id: "pins", label: "Pins" },
  { id: "imports", label: "Imports" },
  { id: "warnings", label: "Warnings/Errors", tone: "warning" },
];

export interface ActivityFiltersProps {
  value: RecentActivityFilter;
  onChange: (value: RecentActivityFilter) => void;
  compact?: boolean;
}

export default function ActivityFilters(props: ActivityFiltersProps) {
  if (!props.compact) {
    return <FilterChips options={OPTIONS} value={props.value} onChange={props.onChange} className="activityFilters" />;
  }

  const activeOption = OPTIONS.find((option) => option.id === props.value) ?? OPTIONS[0];

  return (
    <div className="activityFilters activityFiltersCompactSingle">
      <MenuSelect
        value={props.value}
        labelPrefix="Filter"
        buttonLabel={activeOption.label}
        options={OPTIONS.map((option) => ({ value: option.id, label: option.label }))}
        onChange={(value) => props.onChange(value as RecentActivityFilter)}
        align="start"
        compact
        compactPanelMinWidth={170}
      />
    </div>
  );
}
