import React, { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import Icon from "../Icon";
import usePortalDropdownLayout from "./usePortalDropdownLayout";

export default function MultiSelectDropdown({
  values,
  placeholder,
  groups,
  onChange,
  placement,
  showSearch = true,
  searchPlaceholder = "Search categories…",
  clearLabel = "Clear",
  onClear,
  disabled = false,
  showGroupHeaders = true,
  itemVariant = "drop",
  panelMinWidth = 300,
  panelEstimatedHeight = 420,
  allSelectedLabel,
}: {
  values: string[];
  placeholder: string;
  groups: { group: string; items: { id: string; label: string }[] }[];
  onChange: (v: string[]) => void;
  placement?: "top" | "bottom";
  showSearch?: boolean;
  searchPlaceholder?: string;
  clearLabel?: string;
  onClear?: () => void;
  disabled?: boolean;
  showGroupHeaders?: boolean;
  itemVariant?: "drop" | "menu";
  panelMinWidth?: number;
  panelEstimatedHeight?: number;
  allSelectedLabel?: string;
}) {
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const rootRef = useRef<HTMLDivElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const layout = usePortalDropdownLayout({
    open,
    rootRef,
    placement,
    estimatedHeight: panelEstimatedHeight,
    minWidth: panelMinWidth,
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
        items: g.items.filter(
          (it) => it.id.toLowerCase().includes(qq) || it.label.toLowerCase().includes(qq)
        ),
      }))
      .filter((g) => g.items.length > 0);
  }, [groups, q]);

  const label = useMemo(() => {
    if (!values || values.length === 0) return placeholder;
    const totalOptionCount = groups.reduce((count, group) => count + group.items.length, 0);
    if (allSelectedLabel && totalOptionCount > 0 && values.length >= totalOptionCount) {
      return allSelectedLabel;
    }
    const map = new Map<string, string>();
    for (const g of groups) for (const it of g.items) map.set(it.id, it.label);
    const labels = values.map((v) => map.get(v) ?? v).filter(Boolean);
    if (labels.length === 1) return labels[0];
    if (labels.length === 2) return `${labels[0]}, ${labels[1]}`;
    return `${labels[0]} +${labels.length - 1}`;
  }, [allSelectedLabel, groups, placeholder, values]);

  const toggle = (id: string) => {
    const set = new Set(values);
    if (set.has(id)) set.delete(id);
    else set.add(id);
    onChange(Array.from(set));
  };

  return (
    <div className={`dropdown multiSelectDropdown ${open ? "open" : ""}`} ref={rootRef}>
      <div
        className={`dropBtn ${values.length ? "value" : ""}`}
        onClick={() => {
          if (disabled) return;
          setOpen((o) => !o);
        }}
        style={disabled ? { opacity: 0.6, cursor: "not-allowed" } : undefined}
      >
        <div>{label}</div>
        <span className="dropCaret" aria-hidden="true">
          <Icon name="chevron_down" size={11} />
        </span>
      </div>

      {open && layout
        ? createPortal(
            <div
              ref={panelRef}
              className={`dropPanel portal multiSelectPanel ${layout.placement === "top" ? "top" : ""} ${showSearch ? "hasSearch" : "noSearch"}`}
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
                {showSearch ? (
                  <>
                    <input
                      className="input"
                      value={q}
                      onChange={(e) => setQ(e.target.value)}
                      placeholder={searchPlaceholder}
                      autoFocus
                    />

                    <div style={{ height: 10 }} />
                  </>
                ) : null}

                {filtered.length === 0 ? (
                  <div className="multiSelectEmptyState">No matches</div>
                ) : (
                  filtered.map((g) => (
                    <div key={g.group} className="multiSelectGroup">
                      {showGroupHeaders ? <div className="groupHdr">{g.group}</div> : null}
                      {g.items.map((it) => {
                        const checked = values.includes(it.id);
                        return (
                          <div
                            key={it.id}
                            className={`${itemVariant === "menu" ? "menuItem" : "dropItem"} ${checked ? "active" : ""}`}
                            style={{ display: "flex", alignItems: "center", justifyContent: "space-between" }}
                            onClick={() => toggle(it.id)}
                          >
                            <div style={{ paddingRight: 12 }}>{it.label}</div>
                            <div style={{ opacity: checked ? 1 : 0.35, fontWeight: 1000 }}>{checked ? "✓" : ""}</div>
                          </div>
                        );
                      })}
                    </div>
                  ))
                )}

                <div style={{ height: 8 }} />
                <div className="multiSelectFooter">
                  <button
                    className="dropMiniBtn"
                    onClick={() => {
                      if (onClear) {
                        onClear();
                      } else {
                        onChange([]);
                      }
                      setQ("");
                    }}
                  >
                    {clearLabel}
                  </button>
                  <div className="multiSelectFooterSpacer" />
                  <button
                    className="dropMiniBtn"
                    onClick={() => {
                      setOpen(false);
                      setQ("");
                    }}
                  >
                    Done
                  </button>
                </div>
              </div>
            </div>,
            document.body
          )
        : null}
    </div>
  );
}
