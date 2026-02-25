import React, { type ReactNode } from "react";

export default function NavButton(props: {
  active: boolean;
  label: string;
  onClick: () => void;
  children: ReactNode;
  variant?: "default" | "accent";
  className?: string;
  badge?: number;
}) {
  return (
    <button
      className={`navBtn ${props.active ? "active" : ""} ${props.variant === "accent" ? "accent" : ""} ${props.className ?? ""}`}
      onClick={props.onClick}
    >
      {props.children}
      {(props.badge ?? 0) > 0 ? (
        <span className="navBadge" aria-label={`${props.badge} updates available`}>
          {(props.badge ?? 0) > 99 ? "99+" : props.badge}
        </span>
      ) : null}
      <div className="navTooltip">{props.label}</div>
    </button>
  );
}
