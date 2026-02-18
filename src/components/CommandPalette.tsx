import { useEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";

export type CommandPaletteItem = {
  id: string;
  label: string;
  group: string;
  detail?: string;
  keywords?: string[];
  run: () => void;
};

export default function CommandPalette({
  open,
  title = "Command Palette",
  items,
  onClose,
}: {
  open: boolean;
  title?: string;
  items: CommandPaletteItem[];
  onClose: () => void;
}) {
  const [query, setQuery] = useState("");
  const [selectedIndex, setSelectedIndex] = useState(0);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const panelRef = useRef<HTMLDivElement | null>(null);

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
    const needle = query.trim().toLowerCase();
    if (!needle) return items;
    return items.filter((item) => {
      const haystack = [
        item.label,
        item.group,
        item.detail ?? "",
        ...(item.keywords ?? []),
      ]
        .join(" ")
        .toLowerCase();
      return haystack.includes(needle);
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
        const selected = filtered[selectedIndex];
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

  const grouped = filtered.reduce((acc, item, index) => {
    const key = item.group || "General";
    const list = acc.get(key) ?? [];
    list.push({ item, index });
    acc.set(key, list);
    return acc;
  }, new Map<string, Array<{ item: CommandPaletteItem; index: number }>>());

  return createPortal(
    <div className="commandPaletteOverlay" role="dialog" aria-modal="true" aria-label={title}>
      <div ref={panelRef} className="commandPalettePanel">
        <div className="commandPaletteHeader">{title}</div>
        <input
          ref={inputRef}
          className="input commandPaletteInput"
          placeholder="Search actions and settings…"
          value={query}
          onChange={(event) => {
            setQuery(event.target.value);
            setSelectedIndex(0);
          }}
        />
        <div className="commandPaletteList">
          {grouped.size === 0 ? (
            <div className="commandPaletteEmpty">No matches</div>
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
                        className={`commandPaletteItem ${active ? "active" : ""}`}
                        onMouseEnter={() => setSelectedIndex(index)}
                        onClick={() => {
                          onClose();
                          item.run();
                        }}
                      >
                        <div className="commandPaletteItemLabel">{item.label}</div>
                        {item.detail ? <div className="commandPaletteItemDetail">{item.detail}</div> : null}
                      </button>
                    );
                  })}
                </div>
              </div>
            ))
          )}
        </div>
      </div>
    </div>,
    document.body
  );
}
