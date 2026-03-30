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
  | "chevron_down"
  | "chevron_left"
  | "chevron_right";

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
          <path className="homeRoof" d="M4.2 10.4L12 4l7.8 6.4" />
          <path className="homeBody" d="M6.4 9.2V19a1 1 0 0 0 1 1h9.2a1 1 0 0 0 1-1V9.2" />
          <path className="homeDoor" d="M10.3 20v-4.9a.9.9 0 0 1 .9-.9h1.6a.9.9 0 0 1 .9.9V20" />
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
          <path className="boxShell" d="M21 16V8a2 2 0 0 0-1-1.73L13 2.27a2 2 0 0 0-2 0L4 6.27A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16z" />
          <path className="boxLid" d="M3.3 7l8.7 5 8.7-5" />
          <path className="boxSpine" d="M12 22V12" />
        </svg>
      );
    case "books":
      return (
        <svg {...common}>
          <path className="booksShelf" d="M4.15 19.2A2.65 2.65 0 0 1 6.8 16.55H20" />
          <path className="booksRear booksRearLeft" d="M6.75 1.6H19.02A.98.98 0 0 1 20 2.58v16.62H6.92A2.92 2.92 0 0 1 4 16.28V4.5a2.92 2.92 0 0 1 2.75-2.9z" />
          <path className="booksRear booksRearRight" d="M6.75 1.6H19.02A.98.98 0 0 1 20 2.58v16.62H6.92A2.92 2.92 0 0 1 4 16.28V4.5a2.92 2.92 0 0 1 2.75-2.9z" />
          <path className="booksFrontFill" d="M6.75 1.6H19.02A.98.98 0 0 1 20 2.58v16.62H6.92A2.92 2.92 0 0 1 4 16.28V4.5a2.92 2.92 0 0 1 2.75-2.9z" />
          <path className="booksFront" d="M6.75 1.6H19.02A.98.98 0 0 1 20 2.58v16.62H6.92A2.92 2.92 0 0 1 4 16.28V4.5a2.92 2.92 0 0 1 2.75-2.9z" />
          <path className="bookCornerMark" d="M13.35 2.2v5.8l1.7-1.25L16.75 8V2.2" />
        </svg>
      );
    case "skin":
      return (
        <svg {...common} strokeWidth="2.35">
          <path d="M9.55 5.5A2.45 2.45 0 1 1 14.35 5.5c0 1-.33 1.73-1.12 2.42-.82.73-1.23 1.18-1.23 2.18v1.2" />
          <path d="M12 11.3 19.55 15.2c1.42.73 1.87 2.54.9 3.62-.45.51-1.08.78-1.8.78H5.35c-.72 0-1.35-.27-1.8-.78-.97-1.08-.52-2.89.9-3.62L12 11.3Z" />
        </svg>
      );
    case "bell":
      return (
        <svg {...common}>
          <path className="bellBody" d="M15 17H9c-1.2 0-2-.8-2-2v-3.2a5 5 0 0 1 10 0V15c0 1.2-.8 2-2 2z" />
          <path className="bellStem" d="M12 5V3.5" />
          <path className="bellClapper" d="M10 20a2 2 0 0 0 4 0" />
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
    case "chevron_left":
      return (
        <svg {...common}>
          <path d="M15 6l-6 6 6 6" />
        </svg>
      );
    case "chevron_right":
      return (
        <svg {...common}>
          <path d="M9 6l6 6-6 6" />
        </svg>
      );
    case "download":
      return (
        <svg {...common}>
          <path d="M12 3v12" />
          <path d="M7.4 11.2 12 15.8l4.6-4.6" />
          <path d="M4.9 17.9v1.35A1.75 1.75 0 0 0 6.65 21h10.7a1.75 1.75 0 0 0 1.75-1.75V17.9" />
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
        <svg {...common} strokeWidth="1.55">
          <g className="trashIconLid" transform="translate(0 0.55)">
            <rect x="5.55" y="5.55" width="12.9" height="2.05" rx="1.02" />
            <path d="M10.18 5.32v-.46A1.1 1.1 0 0 1 11.28 3.76h1.44a1.1 1.1 0 0 1 1.1 1.1v.46" />
          </g>
          <g className="trashIconBody" transform="translate(0 0.55)">
            <path d="M7.32 8.18h9.36l-.7 10.5a1.86 1.86 0 0 1-1.86 1.7h-4.24a1.86 1.86 0 0 1-1.86-1.7L7.32 8.18Z" />
            <path d="M9.9 10.55l.26 6.1" strokeWidth="1.22" />
            <path d="M12 10.35v6.3" strokeWidth="1.22" />
            <path d="M14.1 10.55l-.26 6.1" strokeWidth="1.22" />
          </g>
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
          <path className="sparkleMain" d="M12 2l1.2 4.3L17.5 8l-4.3 1.2L12 13.5l-1.2-4.3L6.5 8l4.3-1.7z" />
          <path className="sparkleMinor sparkleMinorA" d="M19 13l.6 2.1L22 16l-2.4.9L19 19l-.6-2.1L16 16l2.4-.9z" />
          <path className="sparkleMinor sparkleMinorB" d="M4.5 13l.5 1.7L7 15l-2 .7-.5 1.8-.5-1.8L2 15l2-.3z" />
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
