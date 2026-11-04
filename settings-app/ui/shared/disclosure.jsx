import React, { useLayoutEffect, useRef, useState } from "react";

function milliseconds(value) {
  return Number.parseFloat(value) * (value.trim().endsWith("ms") ? 1 : 1000);
}

export function DisclosurePanel({ id, expanded, toggle, children, className = "documentation-children" }) {
  const panel = useRef(null);
  const [visible, setVisible] = useState(expanded);
  const [open, setOpen] = useState(expanded);

  useLayoutEffect(() => {
    const element = panel.current;
    const reduced = document.documentElement.dataset.reducedMotion === "true"
      || window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    if (expanded) {
      setVisible(true);
      if (reduced) {
        setOpen(true);
        return;
      }
      const frame = window.requestAnimationFrame(() => {
        element.getBoundingClientRect();
        setOpen(true);
      });
      return () => window.cancelAnimationFrame(frame);
    }
    if (element.contains(document.activeElement)) toggle.current?.focus();
    setOpen(false);
    const style = window.getComputedStyle(element);
    const durations = style.transitionDuration.split(",").map(milliseconds);
    const delays = style.transitionDelay.split(",").map(milliseconds);
    const duration = Math.max(0, ...durations.map((value, index) => value + (delays[index % delays.length] || 0)));
    if (reduced || !duration) {
      setVisible(false);
      return;
    }
    const timer = window.setTimeout(() => setVisible(false), duration + 50);
    return () => window.clearTimeout(timer);
  }, [expanded, toggle]);

  return <div id={id} className={className} data-disclosure-panel="" data-disclosure-state={open ? "open" : "closed"}
    hidden={!visible} inert={!expanded} ref={panel}>
    <div className="documentation-children-inner" data-disclosure-content="">{children}</div>
  </div>;
}
