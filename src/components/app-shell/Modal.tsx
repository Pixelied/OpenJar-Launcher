import React, { useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";

export default function Modal({
  title,
  titleNode,
  onClose,
  children,
  size = "default",
  className,
}: {
  title: string;
  titleNode?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  size?: "default" | "wide" | "xwide";
  className?: string;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "Escape") onClose();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  const modalNode = (
    <div className="modalOverlay" onMouseDown={onClose}>
      <div
        className={`modal ${size === "wide" ? "wide" : ""} ${size === "xwide" ? "xwide" : ""} ${className ?? ""}`}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="modalHeader">
          {titleNode ?? <div className="modalTitle">{title}</div>}
          <button className="iconBtn" onClick={onClose} aria-label="Close">
            ✕
          </button>
        </div>
        {children}
      </div>
    </div>
  );

  if (typeof document === "undefined") return modalNode;
  return createPortal(modalNode, document.body);
}
