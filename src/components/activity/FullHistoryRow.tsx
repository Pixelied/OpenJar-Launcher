import Icon from "../app-shell/Icon";
import type { RecentActivityFeedEntry } from "./types";

export interface FullHistoryRowProps {
  entry: RecentActivityFeedEntry;
  selected?: boolean;
}

export default function FullHistoryRow({ entry, selected = false }: FullHistoryRowProps) {
  return (
    <li className={`fullHistoryRow accent-${entry.accent} ${selected ? "isSelected" : ""}`} data-accent={entry.accent}>
      <span className={`fullHistoryRowIcon accent-${entry.accent}`} aria-hidden="true">
        <Icon name={entry.icon} size={14} />
      </span>
      <div className="fullHistoryRowBody">
        <div className="fullHistoryRowPrimary" title={entry.message}>
          {entry.message}
        </div>
        <div className="fullHistoryRowSecondary" title={entry.target}>
          {entry.target}
        </div>
        <time className="fullHistoryRowMeta" data-oj-tooltip={entry.exactTime}>
          {entry.sourceLabel} • {entry.exactTime}
        </time>
      </div>
    </li>
  );
}
