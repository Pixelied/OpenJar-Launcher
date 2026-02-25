import React, { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import usePortalDropdownLayout from "./usePortalDropdownLayout";

export default function MenuSelect({
  value,
  labelPrefix,
  options,
  onChange,
  placement,
  align,
}: {
  value: string;
  labelPrefix: string;
  options: { value: string; label: string }[];
  onChange: (v: string) => void;
  placement?: "top" | "bottom";
  align?: "start" | "end";
}) {
  const [open, setOpen] = useState(false);
  const rootRef = useRef<HTMLDivElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const layout = usePortalDropdownLayout({
    open,
    rootRef,
    placement,
    estimatedHeight: 260,
    minWidth: 190,
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

  return (
    <div className={`dropdown ${open ? "open" : ""}`} ref={rootRef}>
      <div className="dropBtn value" onClick={() => setOpen((o) => !o)}>
        <div>
          {labelPrefix}: {label}
        </div>
        <div style={{ opacity: 0.7 }}>▾</div>
      </div>

      {open && layout
        ? createPortal(
            <div
              ref={panelRef}
              className={`dropPanel portal ${layout.placement === "top" ? "top" : ""}`}
              style={{
                top: layout.top,
                left: layout.left,
                width: layout.width,
                maxHeight: layout.maxHeight,
                transform: layout.placement === "top" ? "translateY(-100%)" : "none",
              }}
              onMouseDown={(e) => e.stopPropagation()}
            >
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
                  <div className="menuCheck">{o.value === value ? "✓" : ""}</div>
                </div>
              ))}
            </div>,
            document.body
          )
        : null}
    </div>
  );
}
