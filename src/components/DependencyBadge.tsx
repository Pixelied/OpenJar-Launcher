// Dependency badge is intentionally compact and low-saturation so it reads as a signal, not a dominant alert.
export interface DependencyBadgeProps {
  warnings: string[];
  onClick?: () => void;
  busy?: boolean;
}

export default function DependencyBadge(props: DependencyBadgeProps) {
  if (!Array.isArray(props.warnings) || props.warnings.length === 0) return null;
  const label = props.busy ? "Installing deps…" : "Missing deps";
  const countSuffix = props.warnings.length > 1 ? ` (${props.warnings.length})` : "";
  if (!props.onClick) {
    return (
      <span className="instanceProviderBadge warning dependencyBadge" data-oj-tooltip={props.warnings.join(" | ")}>
        <span className="instanceProviderBadgeIcon dependencyBadgeIcon" aria-hidden="true">
          !
        </span>
        {label}
        {countSuffix}
      </span>
    );
  }
  return (
    <button
      type="button"
      className="instanceProviderBadge warning dependencyBadge clickable"
      onClick={(event) => {
        event.stopPropagation();
        props.onClick?.();
      }}
      disabled={props.busy}
      data-oj-tooltip={`${props.warnings.join(" | ")} • Click to install required dependencies.`}
    >
      <span className="instanceProviderBadgeIcon dependencyBadgeIcon" aria-hidden="true">
        !
      </span>
      {label}
      {countSuffix}
    </button>
  );
}
