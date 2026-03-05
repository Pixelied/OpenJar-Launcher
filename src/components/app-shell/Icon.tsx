import React from "react";

export type IconName =
  | "home"
  | "compass"
  | "box"
  | "books"
  | "skin"
  | "bell"
  | "plus"
  | "gear"
  | "user"
  | "search"
  | "x"
  | "play"
  | "download"
  | "sliders"
  | "cpu"
  | "sparkles"
  | "layers"
  | "folder"
  | "upload"
  | "trash"
  | "check_circle"
  | "slash_circle"
  | "chevron_down";

export default function Icon(props: { name: IconName; size?: number; className?: string }) {
  const size = props.size ?? 22;
  const cls = props.className ?? "navIcon";

  const common = {
    xmlns: "http://www.w3.org/2000/svg",
    width: size,
    height: size,
    viewBox: "0 0 24 24",
    fill: "none",
    stroke: "currentColor",
    strokeWidth: "1.9",
    strokeLinecap: "round" as const,
    strokeLinejoin: "round" as const,
    className: cls,
  };

  switch (props.name) {
    case "home":
      return (
        <svg {...common}>
          <path d="M4.2 10.4L12 4l7.8 6.4" />
          <path d="M6.4 9.2V19a1 1 0 0 0 1 1h9.2a1 1 0 0 0 1-1V9.2" />
          <path d="M10.3 20v-4.9a.9.9 0 0 1 .9-.9h1.6a.9.9 0 0 1 .9.9V20" />
        </svg>
      );
    case "compass":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="9" />
          <polygon className="compassNeedle" points="15.6 8.4 13.5 13.5 8.4 15.6 10.5 10.5 15.6 8.4" />
        </svg>
      );
    case "box":
      return (
        <svg {...common}>
          <path d="M21 16V8a2 2 0 0 0-1-1.73L13 2.27a2 2 0 0 0-2 0L4 6.27A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
          <path d="M3.3 7l8.7 5 8.7-5" />
          <path d="M12 22V12" />
        </svg>
      );
    case "books":
      return (
        <svg {...common}>
          <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20" />
          <path d="M6.5 2H19.1A.9.9 0 0 1 20 2.9V22H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z" />
          <path className="bookCornerMark" d="M13.35 2.2v5.8l1.7-1.25L16.75 8V2.2" />
        </svg>
      );
    case "skin":
      return (
        <svg {...common}>
          <rect x="5.2" y="4.8" width="13.6" height="13.6" rx="2.8" />
          <path d="M8.4 10.2h.01" />
          <path d="M15.6 10.2h.01" />
          <path d="M8.2 14.3c1.1.95 2.4 1.45 3.8 1.45s2.7-.5 3.8-1.45" />
        </svg>
      );
    case "bell":
      return (
        <svg {...common}>
          <path d="M15 17H9c-1.2 0-2-.8-2-2v-3.2a5 5 0 0 1 10 0V15c0 1.2-.8 2-2 2z" />
          <path d="M12 5V3.5" />
          <path d="M10 20a2 2 0 0 0 4 0" />
        </svg>
      );
    case "plus":
      return (
        <svg {...common}>
          <path d="M12 5v14" />
          <path d="M5 12h14" />
        </svg>
      );
    case "gear":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="3" />
          <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1 0 2.83 2 2 0 0 1-2.83 0l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09a1.65 1.65 0 0 0-1-1.51 1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06a1.65 1.65 0 0 0 .33-1.82 1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09a1.65 1.65 0 0 0 1.51-1 1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06a1.65 1.65 0 0 0 1.82.33h.09a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06a1.65 1.65 0 0 0-.33 1.82v.09a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z" />
        </svg>
      );
    case "user":
      return (
        <svg {...common}>
          <circle className="userHead" cx="12" cy="8" r="3.4" />
          <path className="userBody" d="M5 19a7 7 0 0 1 14 0" />
        </svg>
      );
    case "search":
      return (
        <svg {...common}>
          <circle cx="11" cy="11" r="7" />
          <path d="M20 20l-3.2-3.2" />
        </svg>
      );
    case "x":
      return (
        <svg {...common}>
          <path d="M18 6L6 18" />
          <path d="M6 6l12 12" />
        </svg>
      );
    case "play":
      return (
        <svg {...common}>
          <path d="M8 5l12 7-12 7z" />
        </svg>
      );
    case "check_circle":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="9" />
          <path d="M8.3 12.2l2.3 2.3 5.1-5.1" />
        </svg>
      );
    case "slash_circle":
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="9" />
          <path d="M8 16L16 8" />
        </svg>
      );
    case "chevron_down":
      return (
        <svg {...common}>
          <path d="M6 9l6 6 6-6" />
        </svg>
      );
    case "download":
      return (
        <svg {...common}>
          <path d="M12 3v12" />
          <path d="M7 10l5 5 5-5" />
          <path d="M5 21h14" />
        </svg>
      );
    case "upload":
      return (
        <svg {...common}>
          <path d="M12 21V9" />
          <path d="M7 14l5-5 5 5" />
          <path d="M5 3h14" />
        </svg>
      );
    case "trash":
      return (
        <svg {...common}>
          <path d="M3 6h18" />
          <path d="M9 6V4.6A1.6 1.6 0 0 1 10.6 3h2.8A1.6 1.6 0 0 1 15 4.6V6" />
          <path d="M6.5 6l.8 12.7A2 2 0 0 0 9.3 20.6h5.4a2 2 0 0 0 2-1.9L17.5 6" />
          <path d="M10 10.2v6.4" />
          <path d="M14 10.2v6.4" />
        </svg>
      );
    case "sliders":
      return (
        <svg {...common}>
          <path d="M4 21v-7" />
          <path d="M4 10V3" />
          <path d="M12 21v-9" />
          <path d="M12 8V3" />
          <path d="M20 21v-5" />
          <path d="M20 12V3" />
          <path d="M2 14h4" />
          <path d="M10 12h4" />
          <path d="M18 16h4" />
        </svg>
      );
    case "cpu":
      return (
        <svg {...common}>
          <rect x="8" y="8" width="8" height="8" rx="1.5" />
          <path d="M12 2v3" />
          <path d="M12 19v3" />
          <path d="M2 12h3" />
          <path d="M19 12h3" />
          <path d="M4.5 4.5l2 2" />
          <path d="M17.5 17.5l2 2" />
          <path d="M19.5 4.5l-2 2" />
          <path d="M4.5 19.5l2-2" />
        </svg>
      );
    case "sparkles":
      return (
        <svg {...common}>
          <path d="M12 2l1.2 4.3L17.5 8l-4.3 1.2L12 13.5l-1.2-4.3L6.5 8l4.3-1.7z" />
          <path d="M19 13l.6 2.1L22 16l-2.4.9L19 19l-.6-2.1L16 16l2.4-.9z" />
          <path d="M4.5 13l.5 1.7L7 15l-2 .7-.5 1.8-.5-1.8L2 15l2-.3z" />
        </svg>
      );
    case "layers":
      return (
        <svg {...common}>
          <path d="M12 2l10 6-10 6L2 8z" />
          <path d="M2 12l10 6 10-6" />
        </svg>
      );
    case "folder":
      return (
        <svg {...common}>
          <path d="M3 7a2 2 0 0 1 2-2h5l2 2h7a2 2 0 0 1 2 2v9a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z" />
        </svg>
      );
    default:
      return (
        <svg {...common}>
          <circle cx="12" cy="12" r="9" />
        </svg>
      );
  }
}
