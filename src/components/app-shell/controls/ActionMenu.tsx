import React, { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import Icon from "../Icon";
import usePortalDropdownLayout from "./usePortalDropdownLayout";

export default function ActionMenu({
  buttonLabel,
  items,
  onAction,
  align,
  compact = false,
  panelMinWidth,
}: {
  buttonLabel: string;
  items: { value: string; label: string; disabled?: boolean }[];
  onAction: (value: string) => void;
  align?: "start" | "end";
  compact?: boolean;
  panelMinWidth?: number;
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const layout = usePortalDropdownLayout({
    open,
    rootRef,
    estimatedHeight: 220,
    minWidth: panelMinWidth ?? (compact ? 180 : 220),
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

  return (
    <div className={`dropdown actionMenu ${compact ? "compact" : ""} ${open ? "open" : ""}`} ref={rootRef}>
      <button type="button" className="dropBtn value actionMenuBtn" onClick={() => setOpen((prev) => !prev)}>
        <div>{buttonLabel}</div>
        <span className="dropCaret" aria-hidden="true">
          <Icon name="chevron_down" size={11} />
        </span>
      </button>

      {open && layout
        ? createPortal(
            <div
              ref={panelRef}
              className={`dropPanel portal actionMenuPanel ${layout.placement === "top" ? "top" : ""}`}
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
                {items.map((item) => (
                  <button
                    key={item.value}
                    type="button"
                    className="menuItem actionMenuItem"
                    disabled={item.disabled}
                    onClick={() => {
                      setOpen(false);
                      window.setTimeout(() => onAction(item.value), 0);
                    }}
                  >
                    <span>{item.label}</span>
                  </button>
                ))}
              </div>
            </div>,
            document.body
          )
        : null}
    </div>
  );
}
