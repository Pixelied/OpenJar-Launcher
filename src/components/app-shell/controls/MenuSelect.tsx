import React, { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import Icon from "../Icon";
import usePortalDropdownLayout from "./usePortalDropdownLayout";

export default function MenuSelect({
  value,
  labelPrefix,
  buttonLabel,
  options,
  onChange,
  placement,
  align,
  compact = false,
  compactPanelMinWidth,
  panelMinWidth,
  panelClassName,
}: {
  value: string;
  labelPrefix: string;
  buttonLabel?: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
  placement?: "top" | "bottom";
  align?: "start" | "end";
  compact?: boolean;
  compactPanelMinWidth?: number;
  panelMinWidth?: number;
  panelClassName?: string;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const longestOptionLabel = useMemo(
    () => options.reduce((max, option) => Math.max(max, option.label.length), 0),
    [options]
  );
  const estimatedMinWidth = useMemo(() => {
    const textEstimate = compact ? longestOptionLabel * 7 + 44 : longestOptionLabel * 8 + 92;
    const callerMin = Math.max(compactPanelMinWidth ?? 0, panelMinWidth ?? 0);
    const baseMin = compact ? 144 : 220;
    const cappedTextEstimate = compact ? textEstimate : Math.min(640, textEstimate);
    return Math.max(baseMin, callerMin, cappedTextEstimate);
  }, [compact, compactPanelMinWidth, longestOptionLabel, panelMinWidth]);

  const layout = usePortalDropdownLayout({
    open,
    rootRef,
    placement,
    estimatedHeight: 260,
    minWidth: estimatedMinWidth,
    align,
  });

  useEffect(() => {
    const onDoc = (e: MouseEvent) => {
      if (!open) return;
      const target = e.target as Node;
      const el = rootRef.current;
      const panel = panelRef.current;
      const path = typeof e.composedPath === "function" ? e.composedPath() : [];
      if (el && path.includes(el)) return;
      if (panel && path.includes(panel)) return;
      if (el?.contains(target) || panel?.contains(target)) return;
      setOpen(false);
    };
    document.addEventListener("mousedown", onDoc);
    return () => document.removeEventListener("mousedown", onDoc);
  }, [open]);

  useEffect(() => {
    if (!open) return;
    const onKey = (e: KeyboardEvent) => {
      if (e.key !== "Escape") return;
      setOpen(false);
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  const label = useMemo(() => {
    const hit = options.find((o) => o.value === value);
    return hit?.label ?? value;
  }, [options, value]);
  const triggerLabel = buttonLabel ?? `${labelPrefix}: ${label}`;

  return (
    <div className={`dropdown menuSelect ${compact ? "compact" : ""} ${open ? "open" : ""}`} ref={rootRef}>
      <div className="dropBtn value menuSelectBtn" onClick={() => setOpen((o) => !o)}>
        <div>{triggerLabel}</div>
        <span className="menuSelectCaret" aria-hidden="true">
          <Icon name="chevron_down" size={11} />
        </span>
      </div>

      {open && layout
        ? createPortal(
            <div
              ref={panelRef}
              className={`dropPanel portal ${layout.placement === "top" ? "top" : ""} ${compact ? "menuSelectCompactPanel" : ""} ${panelClassName ?? ""}`}
              style={{
                top: layout.top,
                left: layout.left,
                width: layout.width,
                maxHeight: layout.maxHeight,
                transform: layout.placement === "top" ? "translateY(-100%)" : "none",
              }}
              onMouseDown={(e) => e.stopPropagation()}
            >
              <div className="dropPanelBody">
                {options.map((o) => (
                  <div
                    key={o.value}
                    className={`menuItem ${o.value === value ? "active" : ""}`}
                    onClick={() => {
                      onChange(o.value);
                      setOpen(false);
                    }}
                  >
                    <div>{o.label}</div>
                    <div
                      className={`menuCheck multiSelectCheck ${o.value === value ? "checked" : ""}`}
                      aria-hidden="true"
                    />
                  </div>
                ))}
              </div>
            </div>,
            document.body
          )
        : null}
    </div>
  );
}
