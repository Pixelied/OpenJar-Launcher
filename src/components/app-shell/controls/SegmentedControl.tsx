import React, { useEffect, useLayoutEffect, useRef, useState } from "react";

export default function SegmentedControl({
  value,
  options,
  onChange,
  variant = "default",
  className,
}: {
  value: string | null;
  options: { value: string | null; label: string }[];
  onChange: (v: string | null) => void;
  variant?: "default" | "scroll";
  className?: string;
}) {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const stretchResetRef = useRef<number | null>(null);
  const previousLeftRef = useRef<number | null>(null);
  const [stretchDirection, setStretchDirection] = useState<"left" | "right" | null>(null);
  const [indicator, setIndicator] = useState<{
    left: number;
    top: number;
    width: number;
    height: number;
    ready: boolean;
  }>({
    left: 0,
    top: 0,
    width: 0,
    height: 0,
    ready: false,
  });

  useEffect(() => {
    return () => {
      if (stretchResetRef.current != null) {
        window.clearTimeout(stretchResetRef.current);
        stretchResetRef.current = null;
      }
    };
  }, []);

  useLayoutEffect(() => {
    const root = rootRef.current;
    if (!root) return;
    const update = () => {
      const active = root.querySelector<HTMLButtonElement>(".segBtn[data-active='true']");
      if (!active) return;
      let left = active.offsetLeft;
      let top = active.offsetTop;
      let width = active.offsetWidth;
      let height = active.offsetHeight;
      if (width < 8 || height < 8) {
        const rootRect = root.getBoundingClientRect();
        const activeRect = active.getBoundingClientRect();
        left = activeRect.left - rootRect.left;
        top = activeRect.top - rootRect.top;
        width = activeRect.width;
        height = activeRect.height;
      }
      const previousLeft = previousLeftRef.current;
      previousLeftRef.current = left;
      if (previousLeft != null && previousLeft !== left) {
        setStretchDirection(left > previousLeft ? "right" : "left");
        if (stretchResetRef.current != null) {
          window.clearTimeout(stretchResetRef.current);
        }
        stretchResetRef.current = window.setTimeout(() => {
          setStretchDirection(null);
          stretchResetRef.current = null;
        }, 260);
      }
      setIndicator((prev) => {
        if (
          prev.left === left &&
          prev.top === top &&
          prev.width === width &&
          prev.height === height &&
          prev.ready
        ) {
          return prev;
        }
        return { left, top, width, height, ready: true };
      });
    };
    update();
    const onResize = () => update();
    const onScroll = () => update();
    window.addEventListener("resize", onResize);
    root.addEventListener("scroll", onScroll, { passive: true });
    return () => {
      window.removeEventListener("resize", onResize);
      root.removeEventListener("scroll", onScroll);
    };
  }, [value, options, variant, className]);

  return (
    <div className={`segmented ${variant === "scroll" ? "scroll" : ""} ${className ?? ""}`} ref={rootRef}>
      <span
        className={`segmentedIndicator ${indicator.ready ? "ready" : ""} ${
          stretchDirection ? `stretch-${stretchDirection}` : ""
        }`}
        style={{
          width: Math.max(0, indicator.width),
          height: Math.max(0, indicator.height),
          transform: `translate(${indicator.left}px, ${indicator.top}px)`,
        }}
        aria-hidden="true"
      />
      {options.map((o) => (
        <button
          key={o.label}
          className={`segBtn ${o.value === value ? "active" : ""}`}
          data-active={o.value === value ? "true" : "false"}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}
