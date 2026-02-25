import { useLayoutEffect, useState } from "react";

type PanelPlacement = "top" | "bottom";

export default function usePortalDropdownLayout({
  open,
  rootRef,
  placement,
  estimatedHeight,
  minWidth,
  align,
}: {
  open: boolean;
  rootRef: { current: HTMLDivElement | null };
  placement?: PanelPlacement;
  estimatedHeight: number;
  minWidth: number;
  align?: "start" | "end";
}) {
  const [layout, setLayout] = useState<{
    top: number;
    left: number;
    width: number;
    maxHeight: number;
    placement: PanelPlacement;
  } | null>(null);

  useLayoutEffect(() => {
    if (!open) {
      setLayout(null);
      return;
    }

    const EDGE = 10;
    const GAP = 10;
    const MIN_HEIGHT = 88;
    const MAX_HEIGHT = 460;

    const update = () => {
      const el = rootRef.current;
      if (!el) return;
      const rect = el.getBoundingClientRect();
      const doc = document.documentElement;
      const viewportWidth = doc?.clientWidth || window.innerWidth;
      const viewportHeight = doc?.clientHeight || window.innerHeight;
      const vw = Math.min(window.innerWidth, viewportWidth);
      const vh = Math.min(window.innerHeight, viewportHeight);

      const spaceBelow = Math.max(0, vh - rect.bottom - EDGE);
      const spaceAbove = Math.max(0, rect.top - EDGE);
      let computedPlacement: PanelPlacement = placement
        ? placement
        : spaceBelow < estimatedHeight && spaceAbove > spaceBelow
          ? "top"
          : "bottom";

      const preferredSpace = computedPlacement === "top" ? spaceAbove : spaceBelow;
      const fallbackSpace = computedPlacement === "top" ? spaceBelow : spaceAbove;
      if (!placement && preferredSpace < 120 && fallbackSpace > preferredSpace + 24) {
        computedPlacement = computedPlacement === "top" ? "bottom" : "top";
      }

      const maxViewportWidth = Math.max(180, vw - EDGE * 2);
      const width = Math.min(Math.max(minWidth, rect.width), maxViewportWidth);
      let left = align === "end" ? rect.right - width : rect.left;
      if (align === "end") left -= 8;
      if (left + width > vw - EDGE) left = Math.max(EDGE, rect.right - width);
      if (left < EDGE) left = EDGE;
      left = Math.min(left, Math.max(EDGE, vw - EDGE - width));

      const availableHeight = computedPlacement === "top" ? spaceAbove : spaceBelow;
      const panelSpace = Math.max(72, availableHeight - GAP);
      const maxHeight = Math.min(MAX_HEIGHT, Math.max(MIN_HEIGHT, panelSpace));
      const top = computedPlacement === "top" ? rect.top - GAP : rect.bottom + GAP;

      setLayout({ top, left, width, maxHeight, placement: computedPlacement });
    };

    update();
    window.addEventListener("resize", update);
    window.addEventListener("scroll", update, true);
    return () => {
      window.removeEventListener("resize", update);
      window.removeEventListener("scroll", update, true);
    };
  }, [open, rootRef, placement, estimatedHeight, minWidth, align]);

  return layout;
}
