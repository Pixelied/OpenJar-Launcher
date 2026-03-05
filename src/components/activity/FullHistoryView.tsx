// Full history view composes lightweight header/list/footer blocks so the modal stays calm while preserving behavior.
import FullHistoryFooter from "./FullHistoryFooter";
import FullHistoryHeader from "./FullHistoryHeader";
import FullHistoryList from "./FullHistoryList";
import type { RecentActivityFeedEntry, RecentActivityFilter } from "./types";

export interface FullHistoryViewProps {
  rows: RecentActivityFeedEntry[];
  filter: RecentActivityFilter;
  onFilterChange: (value: RecentActivityFilter) => void;
  search: string;
  onSearchChange: (value: string) => void;
  busy: boolean;
  hasMore: boolean;
  onRefresh: () => void;
  onLoadOlder: () => void;
  storeLimit: number;
  coalesceWindowMs: number;
}

export default function FullHistoryView(props: FullHistoryViewProps) {
  return (
    <>
      <FullHistoryHeader
        filter={props.filter}
        onFilterChange={props.onFilterChange}
        search={props.search}
        onSearchChange={props.onSearchChange}
      />
      <div className="fullHistoryStoreHint muted" data-oj-tooltip={`Stored history keeps up to ${props.storeLimit} events.`}>
        Showing stored log entries. Older records may age out based on retention.
      </div>
      <FullHistoryList
        rows={props.rows}
        busy={props.busy}
        filter={props.filter}
        coalesceWindowMs={props.coalesceWindowMs}
      />
      <FullHistoryFooter
        busy={props.busy}
        hasMore={props.hasMore}
        onRefresh={props.onRefresh}
        onLoadOlder={props.onLoadOlder}
      />
    </>
  );
}
