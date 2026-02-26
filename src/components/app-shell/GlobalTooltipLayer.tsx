import { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

const CUSTOM_TOOLTIP_ATTR = "data-oj-tooltip";

function tooltipText(value: string | null | undefined): string | null {
  const text = String(value ?? "").trim();
  if (/^loading(\.\.\.|…)?$/i.test(text)) return null;
  return text.length > 0 ? text : null;
}

function prefersReducedMotion() {
  if (typeof window === "undefined" || typeof window.matchMedia !== "function") return false;
  return window.matchMedia("(prefers-reduced-motion: reduce)").matches;
}

function anchorPointForTarget(target: HTMLElement): { x: number; y: number } {
  const rect = target.getBoundingClientRect();
  return {
    x: rect.left + rect.width / 2,
    y: rect.top + rect.height / 2,
  };
}

function promoteElementTitleToCustomTooltip(element: Element) {
  if (!(element instanceof HTMLElement)) return;
  const raw = element.getAttribute("title");
  const text = tooltipText(raw);
  if (!text) return;
  if (!element.getAttribute(CUSTOM_TOOLTIP_ATTR)) {
    element.setAttribute(CUSTOM_TOOLTIP_ATTR, text);
  }
  element.removeAttribute("title");
}

function promoteNativeTitlesInTree(root: ParentNode) {
  if (root instanceof Element && root.hasAttribute("title")) {
    promoteElementTitleToCustomTooltip(root);
  }
  const titleNodes = root.querySelectorAll("[title]");
  for (const node of titleNodes) {
    promoteElementTitleToCustomTooltip(node);
  }
}

function resolveCustomTooltipTarget(source: EventTarget | null): HTMLElement | null {
  if (!(source instanceof Element)) return null;
  const mapped = source.closest(`[${CUSTOM_TOOLTIP_ATTR}]`);
  if (mapped instanceof HTMLElement) {
    // If both native title and custom tooltip are present, always strip title to prevent overlap.
    if (mapped.hasAttribute("title")) {
      promoteElementTitleToCustomTooltip(mapped);
    }
    return mapped;
  }
  const withTitle = source.closest("[title]");
  if (withTitle instanceof HTMLElement) {
    promoteElementTitleToCustomTooltip(withTitle);
    if (withTitle.getAttribute(CUSTOM_TOOLTIP_ATTR)) {
      return withTitle;
    }
  }
  return null;
}

export default function GlobalTooltipLayer() {
  const bubbleRef = useRef<HTMLDivElement | null>(null);
  const activeTargetRef = useRef<HTMLElement | null>(null);
  const openDelayRef = useRef<number | null>(null);
  const closeDelayRef = useRef<number | null>(null);
  const openAnimFrameRef = useRef<number | null>(null);
  const frameRef = useRef<number | null>(null);
  const anchorPointRef = useRef({ x: 0, y: 0 });
  const [tooltip, setTooltip] = useState<{
    visible: boolean;
    open: boolean;
    text: string;
    left: number;
    top: number;
    placement: "above" | "below";
  }>({
    visible: false,
    open: false,
    text: "",
    left: 0,
    top: 0,
    placement: "below",
  });
  const openRef = useRef(false);

  useEffect(() => {
    openRef.current = tooltip.open;
  }, [tooltip.open]);

  useEffect(() => {
    if (typeof document === "undefined") return;
    promoteNativeTitlesInTree(document.body);
    const observer = new MutationObserver((mutations) => {
      for (const mutation of mutations) {
        for (const added of mutation.addedNodes) {
          if (added instanceof Element) {
            promoteNativeTitlesInTree(added);
          }
        }
      }
    });
    observer.observe(document.body, {
      subtree: true,
      childList: true,
      attributes: true,
      attributeFilter: ["title"],
    });
    return () => observer.disconnect();
  }, []);

  useEffect(() => {
    const clearOpenDelay = () => {
      if (openDelayRef.current != null) {
        window.clearTimeout(openDelayRef.current);
        openDelayRef.current = null;
      }
    };
    const clearCloseDelay = () => {
      if (closeDelayRef.current != null) {
        window.clearTimeout(closeDelayRef.current);
        closeDelayRef.current = null;
      }
    };
    const clearOpenAnimFrame = () => {
      if (openAnimFrameRef.current != null) {
        window.cancelAnimationFrame(openAnimFrameRef.current);
        openAnimFrameRef.current = null;
      }
    };
    const resolvePlacement = (point: { x: number; y: number }) => {
      const bubble = bubbleRef.current;
      const bubbleWidth = bubble?.offsetWidth ?? 240;
      const bubbleHeight = bubble?.offsetHeight ?? 34;
      const margin = 10;
      const { x, y } = point;

      let left = Math.max(margin, Math.min(x + 14, window.innerWidth - bubbleWidth - margin));
      let top = y + 18;
      let placement: "above" | "below" = "below";
      if (top + bubbleHeight > window.innerHeight - margin) {
        top = y - bubbleHeight - 14;
        placement = "above";
      }
      if (top < margin) {
        top = margin;
        placement = "below";
      }
      if (left < margin) {
        left = margin;
      }

      return {
        left: Math.round(left),
        top: Math.round(top),
        placement,
      };
    };
    const mountAndOpen = (text: string, point: { x: number; y: number }) => {
      clearOpenAnimFrame();
      setTooltip({
        visible: true,
        open: false,
        text,
        left: point.x,
        top: point.y,
        placement: "below",
      });
      openAnimFrameRef.current = window.requestAnimationFrame(() => {
        openAnimFrameRef.current = window.requestAnimationFrame(() => {
          openAnimFrameRef.current = null;
          const next = resolvePlacement(point);
          setTooltip((prev) =>
            prev.visible
              ? {
                  ...prev,
                  ...next,
                  open: true,
                }
              : prev
          );
        });
      });
    };
    const hide = () => {
      clearOpenDelay();
      clearOpenAnimFrame();
      clearCloseDelay();
      activeTargetRef.current = null;
      if (prefersReducedMotion()) {
        setTooltip((prev) => (prev.visible ? { ...prev, open: false, visible: false } : prev));
        return;
      }
      setTooltip((prev) => (prev.visible ? { ...prev, open: false } : prev));
      closeDelayRef.current = window.setTimeout(() => {
        closeDelayRef.current = null;
        setTooltip((prev) => (prev.visible ? { ...prev, visible: false } : prev));
      }, 260);
    };
    const scheduleLayout = () => {
      if (frameRef.current != null) return;
      frameRef.current = window.requestAnimationFrame(() => {
        frameRef.current = null;
        if (!activeTargetRef.current) return;
        const next = resolvePlacement(anchorPointRef.current);
        setTooltip((prev) => (prev.visible ? { ...prev, ...next } : prev));
      });
    };
    const activate = (
      target: HTMLElement,
      text: string,
      immediate: boolean
    ) => {
      clearOpenDelay();
      clearCloseDelay();
      activeTargetRef.current = target;
      const point = anchorPointForTarget(target);
      anchorPointRef.current = point;
      const openBubble = () => {
        if (activeTargetRef.current !== target) return;
        if (prefersReducedMotion()) {
          const next = resolvePlacement(point);
          setTooltip({
            visible: true,
            open: true,
            text,
            left: next.left,
            top: next.top,
            placement: next.placement,
          });
          return;
        }
        mountAndOpen(text, point);
      };
      if (immediate || prefersReducedMotion()) {
        openBubble();
      } else {
        const delayMs = openRef.current ? 360 : 620;
        openDelayRef.current = window.setTimeout(openBubble, delayMs);
      }
    };

    const onPointerOver = (event: PointerEvent) => {
      const target = resolveCustomTooltipTarget(event.target);
      if (!target) return;
      const text = tooltipText(target.getAttribute(CUSTOM_TOOLTIP_ATTR));
      if (!text) return;
      if (target === activeTargetRef.current) return;
      activate(target, text, false);
    };
    const onPointerOut = (event: PointerEvent) => {
      if (!activeTargetRef.current) return;
      const related = resolveCustomTooltipTarget(event.relatedTarget);
      if (related && related === activeTargetRef.current) return;
      hide();
    };
    const onFocusIn = (event: FocusEvent) => {
      const target = resolveCustomTooltipTarget(event.target);
      if (!target) return;
      const text = tooltipText(target.getAttribute(CUSTOM_TOOLTIP_ATTR));
      if (!text) return;
      activate(
        target,
        text,
        true
      );
    };
    const onFocusOut = (event: FocusEvent) => {
      const related = resolveCustomTooltipTarget(event.relatedTarget);
      if (related && related === activeTargetRef.current) return;
      hide();
    };
    const onViewportChanged = () => {
      if (!openRef.current) return;
      if (activeTargetRef.current) {
        anchorPointRef.current = anchorPointForTarget(activeTargetRef.current);
      }
      scheduleLayout();
    };

    window.addEventListener("pointerover", onPointerOver, true);
    window.addEventListener("pointerout", onPointerOut, true);
    window.addEventListener("focusin", onFocusIn, true);
    window.addEventListener("focusout", onFocusOut, true);
    window.addEventListener("scroll", onViewportChanged, true);
    window.addEventListener("resize", onViewportChanged);

    return () => {
      clearOpenDelay();
      clearCloseDelay();
      clearOpenAnimFrame();
      if (frameRef.current != null) {
        window.cancelAnimationFrame(frameRef.current);
        frameRef.current = null;
      }
      window.removeEventListener("pointerover", onPointerOver, true);
      window.removeEventListener("pointerout", onPointerOut, true);
      window.removeEventListener("focusin", onFocusIn, true);
      window.removeEventListener("focusout", onFocusOut, true);
      window.removeEventListener("scroll", onViewportChanged, true);
      window.removeEventListener("resize", onViewportChanged);
    };
  }, []);

  if (!tooltip.visible || typeof document === "undefined") return null;
  return createPortal(
    <div
      ref={bubbleRef}
      className={`appTooltipBubble ${tooltip.placement} ${tooltip.open ? "open" : ""}`}
      style={{
        left: tooltip.left,
        top: tooltip.top,
      }}
      role="tooltip"
      aria-hidden="true"
    >
      {tooltip.text}
    </div>,
    document.body
  );
}
