import React, { cloneElement, useLayoutEffect, useRef } from "react";
import { Button } from "./settings-controls.jsx";

export function Tabs({ id, label, items, selected, onSelect, className = "advanced-tabs", useChildPanels = false, children }) {
  const strip = useRef(null);
  const buttons = useRef([]);
  const panels = useRef([]);
  const previous = useRef(selected);

  useLayoutEffect(() => {
    const container = strip.current;
    const button = buttons.current[selected];
    const oldIndex = previous.current;
    previous.current = selected;
    let active = true;
    function refresh(animate = false) {
      if (!active || !button?.offsetWidth) return;
      const left = button.offsetLeft;
      const width = button.offsetWidth;
      const ready = container.classList.contains("indicator-ready");
      // Observer callbacks with unchanged geometry must leave a running transition alone.
      if (!ready || container.style.getPropertyValue("--tab-left") !== left + "px"
        || container.style.getPropertyValue("--tab-width") !== width + "px") {
        const immediate = !animate || !ready;
        if (immediate) {
          container.classList.add("indicator-initializing");
          void container.offsetWidth;
        }
        container.style.setProperty("--tab-left", left + "px");
        container.style.setProperty("--tab-width", width + "px");
        container.classList.add("indicator-ready");
        if (immediate) {
          void container.offsetWidth;
          container.classList.remove("indicator-initializing");
        }
      }
      if (left < container.scrollLeft) container.scrollLeft = left;
      else if (left + width > container.scrollLeft + container.clientWidth) {
        container.scrollLeft = left + width - container.clientWidth;
      }
    }
    const changed = oldIndex !== selected;
    refresh(changed);
    if (changed) {
      const page = container.closest(".page-content");
      if (page) page.scrollTop = 0;
      if (panels.current[oldIndex]?.contains(document.activeElement)) button?.focus({ preventScroll: true });
    }
    const observer = typeof ResizeObserver === "function" ? new ResizeObserver(() => refresh()) : null;
    observer?.observe(container);
    const resize = () => refresh();
    window.addEventListener("resize", resize);
    document.fonts?.ready.then(resize);
    return () => {
      active = false;
      observer?.disconnect();
      window.removeEventListener("resize", resize);
    };
  }, [selected]);

  function move(event, index) {
    const { key } = event;
    if (!["ArrowLeft", "ArrowRight", "Home", "End"].includes(key)) return;
    event.preventDefault();
    const count = items.length;
    const next = key === "Home" ? 0 : key === "End" ? count - 1
      : (index + (key === "ArrowRight" ? 1 : -1) + count) % count;
    buttons.current[next]?.focus({ preventScroll: true });
    onSelect(next);
  }

  return <>
    <div id={id} className={className} role="tablist" aria-label={label} ref={strip}>
      {items.map(({ tabId, panelId, title }, index) =>
        <Button key={tabId} ref={node => { buttons.current[index] = node; }}
          id={tabId} role="tab" aria-selected={index === selected} aria-controls={panelId}
          tabIndex={index === selected ? 0 : -1} onClick={() => onSelect(index)}
          onKeyDown={event => move(event, index)}>{title}</Button>
      )}
    </div>
    {items.map(({ tabId, panelId, panelClassName }, index) => {
      const props = {
        id: panelId, role: "tabpanel", "aria-labelledby": tabId, hidden: index !== selected,
        ref: node => { panels.current[index] = node; },
      };
      if (useChildPanels) return cloneElement(children[index], { ...props, key: panelId });
      return <section key={panelId} className={panelClassName} {...props}>{children[index]}</section>;
    })}
  </>;
}
