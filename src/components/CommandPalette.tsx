import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import Icon, { type IconName } from "./app-shell/Icon";

export type CommandPaletteItem = {
  id: string;
  label: string;
  group: string;
  detail?: string;
  keywords?: string[];
  icon?: IconName;
  badge?: string;
  run: () => void;
};

function scoreCommandPaletteItem(item: CommandPaletteItem, query: string) {
  const needle = query.trim().toLowerCase();
  if (!needle) return 0;

  const tokens = needle.split(/\s+/).filter(Boolean);
  if (tokens.length === 0) return 0;

  const label = item.label.toLowerCase();
  const group = item.group.toLowerCase();
  const detail = String(item.detail ?? "").toLowerCase();
  const keywords = (item.keywords ?? []).join(" ").toLowerCase();

  let score = 0;
  for (const token of tokens) {
    if (label === token) {
      score += 140;
      continue;
    }
    if (label.startsWith(token)) {
      score += 96;
      continue;
    }
    if (label.includes(token)) {
      score += 64;
      continue;
    }
    if (keywords.includes(token)) {
      score += 40;
      continue;
    }
    if (detail.includes(token)) {
      score += 24;
      continue;
    }
    if (group.includes(token)) {
      score += 16;
      continue;
    }
    return -1;
  }

  return score;
}

export default function CommandPalette({
  open,
  title = "Command Palette",
  contextLabel,
  items,
  onClose,
}: {
  open: boolean;
  title?: string;
  contextLabel?: string;
  items: CommandPaletteItem[];
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);
  const itemRefs = useRef<Map<string, HTMLButtonElement>>(new Map());

  useEffect(() => {
    if (!open) {
      setQuery("");
      setSelectedIndex(0);
      return;
    }
    const t = window.setTimeout(() => inputRef.current?.focus(), 0);
    return () => window.clearTimeout(t);
  }, [open]);

  const filtered = useMemo(() => {
    return items
      .map((item, index) => ({
        item,
        score: scoreCommandPaletteItem(item, query),
        index,
      }))
      .filter((entry) => entry.score >= 0)
      .sort((a, b) => {
        if (b.score !== a.score) return b.score - a.score;
        if (a.item.group !== b.item.group) return a.item.group.localeCompare(b.item.group);
        if (a.item.label !== b.item.label) return a.item.label.localeCompare(b.item.label);
        return a.index - b.index;
      });
  }, [items, query]);

  useEffect(() => {
    if (!open) return;
    setSelectedIndex((prev) => {
      if (filtered.length === 0) return 0;
      return Math.max(0, Math.min(filtered.length - 1, prev));
    });
  }, [filtered, open]);

  useEffect(() => {
    if (!open) return;
    const selected = filtered[selectedIndex]?.item;
    if (!selected) return;
    itemRefs.current.get(selected.id)?.scrollIntoView({ block: "nearest" });
  }, [filtered, open, selectedIndex]);

  useEffect(() => {
    if (!open) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        onClose();
        return;
      }
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelectedIndex((prev) => {
          if (filtered.length === 0) return 0;
          return (prev + 1) % filtered.length;
        });
        return;
      }
      if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelectedIndex((prev) => {
          if (filtered.length === 0) return 0;
          return (prev - 1 + filtered.length) % filtered.length;
        });
        return;
      }
      if (event.key === "Enter") {
        if (filtered.length === 0) return;
        event.preventDefault();
        const selected = filtered[selectedIndex]?.item;
        if (!selected) return;
        onClose();
        selected.run();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [filtered, onClose, open, selectedIndex]);

  useEffect(() => {
    if (!open) return;
    const onDocMouseDown = (event: MouseEvent) => {
      const panel = panelRef.current;
      if (!panel) return;
      const target = event.target as Node;
      if (panel.contains(target)) return;
      onClose();
    };
    document.addEventListener("mousedown", onDocMouseDown);
    return () => document.removeEventListener("mousedown", onDocMouseDown);
  }, [onClose, open]);

  if (!open) return null;

  const selectedItem = filtered[selectedIndex]?.item ?? null;
  const grouped = filtered.reduce((acc, entry, filteredIndex) => {
    const key = entry.item.group || "General";
    const list = acc.get(key) ?? [];
    list.push({ item: entry.item, index: filteredIndex });
    acc.set(key, list);
    return acc;
  }, new Map<string, Array<{ item: CommandPaletteItem; index: number }>>());

  return createPortal(
    <div className="commandPaletteOverlay" role="dialog" aria-modal="true" aria-label={title}>
      <div ref={panelRef} className="commandPalettePanel">
        <div className="commandPaletteTop">
          <div className="commandPaletteHeader">Quick access</div>
          <div className="commandPaletteTitleRow">
            <div>
              <div className="commandPaletteTitle">{title}</div>
              <div className="commandPaletteSub">
                {contextLabel
                  ? `Context: ${contextLabel}`
                  : "Search launcher actions, settings, and instance shortcuts."}
              </div>
            </div>
            <div className="commandPaletteMeta">
              {contextLabel ? <span className="commandPaletteMetaPill emphasis">{contextLabel}</span> : null}
              <span className="commandPaletteMetaPill">{filtered.length} results</span>
            </div>
          </div>
        </div>
        <label className="commandPaletteSearchShell">
          <span className="commandPaletteSearchIcon">
            <Icon name="search" size={16} className="commandPaletteSearchSvg" />
          </span>
          <input
            ref={inputRef}
            className="input commandPaletteInput"
            placeholder="Search actions, routes, settings, or instances…"
            value={query}
            onChange={(event) => {
              setQuery(event.target.value);
              setSelectedIndex(0);
            }}
          />
        </label>
        {selectedItem ? (
          <div className="commandPaletteSelectionHint">
            <span className="commandPaletteSelectionLabel">Ready</span>
            <strong>{selectedItem.label}</strong>
            {selectedItem.detail ? <span>{selectedItem.detail}</span> : null}
          </div>
        ) : null}
        <div className="commandPaletteList">
          {grouped.size === 0 ? (
            <div className="commandPaletteEmpty">
              <div className="commandPaletteEmptyTitle">No matches</div>
              <div className="commandPaletteEmptySub">
                Try an instance name, setting, provider, or action like launch, logs, or Java.
              </div>
            </div>
          ) : (
            Array.from(grouped.entries()).map(([group, rows]) => (
              <div key={group} className="commandPaletteGroup">
                <div className="commandPaletteGroupTitle">{group}</div>
                <div className="commandPaletteGroupItems">
                  {rows.map(({ item, index }) => {
                    const active = index === selectedIndex;
                    return (
                      <button
                        key={item.id}
                        ref={(node) => {
                          if (node) {
                            itemRefs.current.set(item.id, node);
                          } else {
                            itemRefs.current.delete(item.id);
                          }
                        }}
                        className={`commandPaletteItem ${active ? "active" : ""}`}
                        onMouseEnter={() => setSelectedIndex(index)}
                        onClick={() => {
                          onClose();
                          item.run();
                        }}
                      >
                        <div className="commandPaletteItemIcon">
                          <Icon
                            name={item.icon ?? "sparkles"}
                            size={16}
                            className="commandPaletteItemIconSvg"
                          />
                        </div>
                        <div className="commandPaletteItemBody">
                          <div className="commandPaletteItemMain">
                            <div className="commandPaletteItemLabel">{item.label}</div>
                            {item.badge ? <span className="commandPaletteItemBadge">{item.badge}</span> : null}
                          </div>
                          {item.detail ? <div className="commandPaletteItemDetail">{item.detail}</div> : null}
                        </div>
                        <div className="commandPaletteItemArrow">↵</div>
                      </button>
                    );
                  })}
                </div>
              </div>
            ))
          )}
        </div>
        <div className="commandPaletteFooter">
          <span className="commandPaletteKeyHint">
            <kbd>↑</kbd>
            <kbd>↓</kbd>
            Move
          </span>
          <span className="commandPaletteKeyHint">
            <kbd>Enter</kbd>
            Run
          </span>
          <span className="commandPaletteKeyHint">
            <kbd>Esc</kbd>
            Close
          </span>
        </div>
      </div>
    </div>,
    document.body
  );
}
