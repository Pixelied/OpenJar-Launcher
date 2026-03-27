import ActivityFilters from "./ActivityFilters";
import type { RecentActivityFilter } from "./types";

export interface FullHistoryHeaderProps {
  filter: RecentActivityFilter;
  onFilterChange: (value: RecentActivityFilter) => void;
  search: string;
  onSearchChange: (value: string) => void;
}

export default function FullHistoryHeader(props: FullHistoryHeaderProps) {
  return (
    <header className="fullHistoryHeader">
      <p className="fullHistorySubtitle">Complete event log with pagination and filters.</p>
      <ActivityFilters value={props.filter} onChange={props.onFilterChange} compact />
      <input
        className="input fullHistorySearch"
        aria-label="Search full history"
        placeholder="Search summary or target..."
        value={props.search}
        onChange={(event) => props.onSearchChange(event.target.value)}
      />
    </header>
  );
}
