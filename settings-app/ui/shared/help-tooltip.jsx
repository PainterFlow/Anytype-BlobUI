import React, { useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import "../help-tooltip.css";

const popoverGap = 7;

export function HelpTooltip({ label, text, size = "normal", icon = null }) {
  const tooltipId = `help-tooltip-${useId()}`;
  const rootRef = useRef(null);
  const popoverRef = useRef(null);
  const [hovered, setHovered] = useState(false);
  const [focused, setFocused] = useState(false);
  const open = hovered || focused;

  useLayoutEffect(() => {
    const root = rootRef.current;
    const popover = popoverRef.current;
    if (!root || !popover) return;
    if (!open) {
      root.removeAttribute("data-positioned");
      popover.removeAttribute("data-positioned");
      return;
    }

    const panel = root.closest('[role="tabpanel"], .component-catalog-card, .settings-content');
    const scrollPane = root.closest(".page-content, .component-catalog-scroll");

    const placePopover = () => {
      const anchor = root.getBoundingClientRect();
      const row = root.parentElement?.getBoundingClientRect() ?? anchor;
      const panelRect = panel?.getBoundingClientRect();
      const paneRect = scrollPane?.getBoundingClientRect();
      const left = Math.max(panelRect?.left ?? 0, paneRect?.left ?? 0);
      const right = Math.min(panelRect?.right ?? window.innerWidth, paneRect?.right ?? window.innerWidth, window.innerWidth);
      const top = Math.max(paneRect?.top ?? 0, 0);
      const bottom = Math.min(paneRect?.bottom ?? window.innerHeight, window.innerHeight);
      const inView = anchor.width > 0 && anchor.right > left && anchor.left < right
        && anchor.bottom > top && anchor.top < bottom;
      root.toggleAttribute("data-positioned", inView);
      popover.toggleAttribute("data-positioned", inView);
      if (!inView) return;

      popover.style.setProperty("--help-max-width", `${right - left}px`);
      const width = popover.offsetWidth;
      const height = popover.offsetHeight;
      let x = anchor.right + popoverGap;
      let y = anchor.top + (anchor.height - height) / 2;

      if (x + width > right) {
        const before = anchor.left - popoverGap - width;
        if (before >= left) {
          x = before;
        } else {
          x = Math.max(left, Math.min(anchor.left, right - width));
          const below = row.bottom + popoverGap;
          const above = row.top - popoverGap - height;
          y = below + height <= bottom || above < top ? below : above;
        }
      }

      popover.style.setProperty("--help-left", `${x}px`);
      popover.style.setProperty("--help-top", `${Math.max(top, Math.min(y, bottom - height))}px`);
    };

    placePopover();
    const resizeObserver = typeof ResizeObserver === "function" ? new ResizeObserver(placePopover) : null;
    for (const element of [root.parentElement, panel, scrollPane]) {
      if (element) resizeObserver?.observe(element);
    }
    window.addEventListener("resize", placePopover);
    scrollPane?.addEventListener("scroll", placePopover, { passive: true });
    return () => {
      resizeObserver?.disconnect();
      window.removeEventListener("resize", placePopover);
      scrollPane?.removeEventListener("scroll", placePopover);
    };
  }, [open, text]);

  return <>
    <span ref={rootRef} className={`help-tooltip${size === "large" ? " help-tooltip-large" : ""}`} data-open={open}
      onPointerEnter={() => setHovered(true)} onPointerLeave={() => setHovered(false)}>
      <button type="button" className="help-tooltip-trigger" aria-label={label} aria-describedby={tooltipId}
        onFocus={() => setFocused(true)} onBlur={() => setFocused(false)}>
        {icon || <svg viewBox="0 0 20 20" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.35" strokeLinecap="round" strokeLinejoin="round">
          <circle cx="10" cy="10" r="8.25" />
          <path d="M7.65 7.45a2.37 2.37 0 0 1 4.7.4c0 1.65-2.35 1.8-2.35 3.45" />
          <path d="M10 14.2h.01" strokeWidth="1.65" />
        </svg>}
      </button>
    </span>
    {createPortal(<span ref={popoverRef} id={tooltipId} className="help-tooltip-popover" role="tooltip" data-open={open}>{text}</span>, document.body)}
  </>;
}
