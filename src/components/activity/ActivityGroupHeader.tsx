// Keeps time bucket separators intentionally subtle so item rows remain the primary focus.
export interface ActivityGroupHeaderProps {
  label: string;
}

export default function ActivityGroupHeader({ label }: ActivityGroupHeaderProps) {
  return <div className="activityGroupHeader">{label}</div>;
}
