export interface FullHistoryFooterProps {
  busy: boolean;
  hasMore: boolean;
  onRefresh: () => void;
  onLoadOlder: () => void;
}

export default function FullHistoryFooter(props: FullHistoryFooterProps) {
  return (
    <div className="footerBar fullHistoryFooter">
      <button className="btn ghost" onClick={props.onRefresh} disabled={props.busy}>
        {props.busy ? "Refreshing…" : "Refresh"}
      </button>
      <button className="btn" onClick={props.onLoadOlder} disabled={props.busy || !props.hasMore}>
        {props.busy ? "Loading…" : props.hasMore ? "Load older events" : "No more events"}
      </button>
    </div>
  );
}
