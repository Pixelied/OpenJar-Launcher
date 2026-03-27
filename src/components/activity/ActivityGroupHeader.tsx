export interface ActivityGroupHeaderProps {
  label: string;
}

export default function ActivityGroupHeader({ label }: ActivityGroupHeaderProps) {
  return <div className="activityGroupHeader">{label}</div>;
}
