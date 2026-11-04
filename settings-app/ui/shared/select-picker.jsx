import React, { useEffect, useId, useLayoutEffect, useRef, useState } from "react";
import { createPortal, flushSync } from "react-dom";

function reducedMotion() {
  return document.documentElement.dataset.reducedMotion === "true" ||
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

export function SelectPicker({
  id, rootId, menuId, label, labelId, valueId, descriptionId, options, value, caption,
  onChange, multiple = false, closeOnSelect = !multiple, disabled = false, inactive = false, emptyMessage,
  icon, pressed, title, portalRef, fixed = false, rootClassName = "", menuClassName = "",
  triggerClassName = "", preservePointerFocus = false, restoreFocusOnSelect = true,
}) {
  const generatedId = useId();
  const listId = menuId || `picker-${generatedId}`;
  const [state, setState] = useState("closed");
  const root = useRef(null);
  const trigger = useRef(null);
  const menu = useRef(null);
  const previousOptions = useRef(options);
  const open = state === "opening" || state === "open";
  const selected = multiple ? value : [value];
  const display = caption ?? options.find(option => option.value === value)?.label ?? "";
  const portal = portalRef?.current;

  function focusOption(index) {
    const items = menu.current?.querySelectorAll('[role="option"]');
    if (!items?.length) { trigger.current?.focus({ preventScroll: true }); return; }
    const item = items[(index + items.length) % items.length];
    item.focus({ preventScroll: true });
    const bounds = menu.current.getBoundingClientRect();
    const rect = item.getBoundingClientRect();
    if (rect.top < bounds.top) menu.current.scrollTop -= bounds.top - rect.top;
    if (rect.bottom > bounds.bottom) menu.current.scrollTop += rect.bottom - bounds.bottom;
  }

  function close(restoreFocus = false) {
    if (!open) return;
    if (restoreFocus) {
      const target = trigger.current?.disabled ? document.getElementById("page-title") : trigger.current;
      target?.focus({ preventScroll: true });
    }
    setState(reducedMotion() ? "closed" : "closing");
  }

  function openMenu() {
    if (!disabled && !inactive) flushSync(() => setState("opening"));
  }

  useLayoutEffect(() => {
    if (!open || disabled || !menu.current) return;
    const button = trigger.current.getBoundingClientRect();
    const list = menu.current;
    const frame = portal?.getBoundingClientRect();
    const margin = 8;
    const availableWidth = (frame?.width ?? window.innerWidth) - margin * 2;
    const width = Math.min(list.offsetWidth, availableWidth);
    list.style.maxWidth = `${availableWidth}px`;
    const below = window.innerHeight - button.bottom - margin - 6;
    const above = button.top - margin - 6;
    const up = below < list.scrollHeight && above > below;
    const maxHeight = Math.max(34, Math.min(360, up ? above : below));
    list.style.maxHeight = `${maxHeight}px`;
    if (frame || fixed) {
      list.style.right = "auto";
      list.style.left = `${Math.max(margin, Math.min(button.right - (frame?.left ?? 0) - width, availableWidth + margin - width))}px`;
      const top = up ? button.top - Math.min(list.offsetHeight, maxHeight) - 6 : button.bottom + 6;
      list.style.top = `${top - (frame?.top ?? 0)}px`;
    } else {
      list.style.top = up ? "auto" : "";
      list.style.bottom = up ? "calc(100% + 6px)" : "";
    }
    if (state === "opening" || (previousOptions.current !== options && !list.contains(document.activeElement)
      && document.activeElement === document.body)) {
      focusOption(Math.max(0, options.findIndex(option => selected.includes(option.value))));
    }
    previousOptions.current = options;
  });

  useLayoutEffect(() => {
    if (state === "opening") {
      if (reducedMotion()) {
        setState("open");
        return;
      }
      const frame = requestAnimationFrame(() => setState("open"));
      return () => cancelAnimationFrame(frame);
    }
    if (state === "closing") {
      const timer = setTimeout(() => setState("closed"), 100);
      return () => clearTimeout(timer);
    }
  }, [state]);

  useLayoutEffect(() => {
    if (disabled || inactive) close(menu.current?.contains(document.activeElement));
  }, [disabled, inactive, open]);

  useEffect(() => {
    if (!open) return;
    const dismiss = event => {
      if (!root.current?.contains(event.target) && !menu.current?.contains(event.target)) close();
    };
    const reposition = event => {
      if (!menu.current?.contains(event.target)) close(menu.current?.contains(document.activeElement));
    };
    document.addEventListener("pointerdown", dismiss);
    document.addEventListener("focusin", dismiss);
    window.addEventListener("resize", reposition);
    document.addEventListener("scroll", reposition, true);
    return () => {
      document.removeEventListener("pointerdown", dismiss);
      document.removeEventListener("focusin", dismiss);
      window.removeEventListener("resize", reposition);
      document.removeEventListener("scroll", reposition, true);
    };
  }, [open, disabled]);

  useEffect(() => {
    if (!open || !menu.current || !window.BlobUI?.Scrollbar) return;
    const scrollbar = window.BlobUI.Scrollbar.attach(menu.current);
    return () => scrollbar.destroy();
  }, [open]);

  function handleKey(event) {
    const { key } = event;
    if (disabled || inactive) return;
    if (key === "Escape" && open) {
      event.preventDefault();
      close(true);
    } else if (key === "Tab" && open) {
      close(true);
    } else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(key)) {
      event.preventDefault();
      if (!open) {
        openMenu();
        if (key === "Home") focusOption(0);
        if (key === "End") focusOption(options.length - 1);
        return;
      }
      const index = options.findIndex(option => option.value === document.activeElement?.dataset.value);
      if (key === "Home") focusOption(0);
      else if (key === "End") focusOption(options.length - 1);
      else focusOption(index < 0 ? key === "ArrowDown" ? 0 : options.length - 1 : index + (key === "ArrowDown" ? 1 : -1));
    }
  }

  function choose(option) {
    const next = multiple
      ? selected.includes(option.value) ? selected.filter(item => item !== option.value) : [...selected, option.value]
      : option.value;
    const shouldClose = onChange?.(next) !== false && closeOnSelect;
    if (shouldClose) close(restoreFocusOnSelect);
  }

  const list = <div ref={menu} id={listId} className={`kanban-select-menu app-scrollbar ${menuClassName}`}
    role="listbox" aria-label={label} aria-multiselectable={multiple || undefined}
    data-state={state} hidden={state === "closed"} inert={!open}
    style={fixed ? { position: "fixed" } : undefined}>
    {options.map(option => <button key={option.value} type="button" className="kanban-select-option"
      role="option" tabIndex={-1} data-value={option.value} aria-label={option.label}
      aria-selected={selected.includes(option.value)} onClick={() => choose(option)}>
      <span className="kanban-option-copy"><span className="kanban-option-label">{option.label}</span>
        {option.description && <span className="kanban-select-option-description">{option.description}</span>}
      </span>
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3.5 8 3 3 6-6" /></svg>
    </button>)}
    {!options.length && emptyMessage && <div className="kanban-select-empty" role="status">{emptyMessage}</div>}
  </div>;

  return <div ref={root} id={rootId} className={`kanban-select ${rootClassName}`} onKeyDown={handleKey}>
    <button ref={trigger} id={id} type="button"
      className={`${icon ? "neutral-icon-button" : "kanban-select-trigger"} ${triggerClassName}`}
      aria-label={labelId ? undefined : label} aria-labelledby={labelId ? `${labelId} ${valueId}` : undefined}
      aria-describedby={descriptionId} aria-pressed={pressed} title={title}
      aria-haspopup="listbox" aria-controls={listId} aria-expanded={open}
      disabled={disabled} value={multiple ? undefined : value}
      onMouseDown={preservePointerFocus ? event => event.preventDefault() : undefined}
      onClick={() => open ? flushSync(() => close()) : openMenu()}>
      {icon ?? <>{labelId && <span id={labelId} className="sr-only">{label}</span>}
        <span id={valueId} className="kanban-trigger-value">{display}</span>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4 6 4 4 4-4" /></svg></>}
    </button>
    {portal ? createPortal(list, portal) : list}
  </div>;
}
