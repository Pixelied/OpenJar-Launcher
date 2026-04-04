import React, { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import Icon from "../Icon";
import usePortalDropdownLayout from "./usePortalDropdownLayout";

type VersionItemLike = {
  id: string;
  label?: string;
  meta?: string;
};

export default function Dropdown({
  value,
  placeholder,
  groups,
  onPick,
  placement,
  includeAny = false,
}: {
  value: string | null;
  placeholder: string;
  groups: { group: string; items: VersionItemLike[] }[];
  onPick: (id: string | null) => void;
  placement?: "top" | "bottom";
  includeAny?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const rootRef = useRef<HTMLDivElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const layout = usePortalDropdownLayout({
    open,
    rootRef,
    placement,
    estimatedHeight: 380,
    minWidth: 280,
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
      setQ("");
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [open]);

  const filtered = useMemo(() => {
    const qq = q.trim().toLowerCase();
    if (!qq) return groups;
    return groups
      .map((g) => ({
        group: g.group,
        items: g.items.filter((it) => {
          const haystack = `${it.id} ${it.label ?? ""} ${it.meta ?? ""}`.toLowerCase();
          return haystack.includes(qq);
        }),
      }))
      .filter((g) => g.items.length > 0);
  }, [groups, q]);

  return (
    <div className={`dropdown ${open ? "open" : ""}`} ref={rootRef}>
      <div className={`dropBtn ${value ? "value" : ""}`} onClick={() => setOpen((o) => !o)}>
        <div>{value ?? placeholder}</div>
        <span className="dropCaret" aria-hidden="true">
          <Icon name="chevron_down" size={11} />
        </span>
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
              <div className="dropPanelBody">
                <input
                  className="input"
                  value={q}
                  onChange={(e) => setQ(e.target.value)}
                  placeholder="Search versions…"
                  autoFocus
                />

                <div style={{ height: 10 }} />

                {filtered.length === 0 ? (
                  <div style={{ padding: 10, color: "var(--muted)", fontWeight: 900 }}>No matches</div>
                ) : (
                  <>
                    {includeAny ? (
                      <div>
                        <div className="groupHdr">General</div>
                        <div
                          className={`dropItem ${value === null ? "active" : ""}`}
                          onClick={() => {
                            onPick(null);
                            setOpen(false);
                            setQ("");
                          }}
                        >
                          Any
                        </div>
                      </div>
                    ) : null}
                    {filtered.map((g) => (
                      <div key={g.group}>
                        <div className="groupHdr">{g.group}</div>
                        {g.items.map((it) => (
                          <div
                            key={it.id}
                            className={`dropItem ${it.id === value ? "active" : ""}`}
                            onClick={() => {
                              onPick(it.id);
                              setOpen(false);
                              setQ("");
                            }}
                          >
                            <div className="dropItemCopy">
                              <div className="dropItemTitle">{it.label ?? it.id}</div>
                              {it.meta ? <div className="dropItemMeta">{it.meta}</div> : null}
                            </div>
                          </div>
                        ))}
                      </div>
                    ))}
                  </>
                )}
              </div>
            </div>,
            document.body
          )
        : null}
    </div>
  );
}
