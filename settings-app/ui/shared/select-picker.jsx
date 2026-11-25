import React, { useEffect, useId, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal, flushSync } from "react-dom";

function reducedMotion() {
  return document.documentElement.dataset.reducedMotion === "true" ||
    window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
}

function asArray(value, multiple) {
  return multiple ? (Array.isArray(value) ? value : []) : [value];
}

let anytypeIconMarkup = null;
let anytypeIconRequest = null;
function loadAnytypeIconMarkup() {
  anytypeIconRequest ||= import("../anytype-icons.js").then(module => {
    anytypeIconMarkup = module.default || {};
    return anytypeIconMarkup;
  }).catch(() => ({}));
  return anytypeIconRequest;
}

function NamedTypeIcon({ name, color }) {
  const key = String(name || "").toLowerCase();
  const colorMap = { grey: "#8f8f8f", yellow: "#ecd91a", orange: "#f0a820", red: "#f25040", pink: "#f040a0", purple: "#c870e8", blue: "#6878ee", ice: "#40b8f5", teal: "#20c8b8", lime: "#4ecc4e" };
  const fill = colorMap[color] || "currentColor";
  const [markup, setMarkup] = useState(() => anytypeIconMarkup?.[key] || "");
  useEffect(() => {
    if (markup) return undefined;
    let active = true;
    loadAnytypeIconMarkup().then(icons => {
      if (active) setMarkup(icons[key] || "");
    });
    return () => { active = false; };
  }, [key, markup]);
  if (!markup) return <svg viewBox="0 0 512 512" aria-hidden="true" fill={fill}><path d="M256 48a208 208 0 1 0 0 416 208 208 0 0 0 0-416Zm0 352a144 144 0 1 1 0-288 144 144 0 0 1 0 288Z" /></svg>;
  return <svg viewBox="0 0 512 512" aria-hidden="true" fill={fill} dangerouslySetInnerHTML={{ __html: markup }} />;
}

export function SelectorOptionIcon({ icon }) {
  if (!icon) return null;
  if (React.isValidElement(icon)) return icon;
  if (typeof icon === "string") {
    if (icon.startsWith("data:image/")) return <img src={icon} alt="" draggable="false" />;
    return <span className="selector-option-emoji">{icon}</span>;
  }
  const source = icon.src || icon.dataUrl || icon.data;
  if (source) return <img src={source} alt="" draggable="false" />;
  const emoji = icon.emoji || (icon.format === "emoji" ? icon.value : "");
  if (emoji) return <span className="selector-option-emoji">{emoji}</span>;
  const name = icon.name || icon.value || "";
  if (name) return <NamedTypeIcon name={name} color={icon.color} />;
  return null;
}

function normalizeSections(options = [], sections = null, query = "") {
  const grouped = [];
  if (Array.isArray(sections) && sections.length) {
    for (const section of sections) {
      grouped.push({
        id: String(section.id ?? section.label ?? grouped.length),
        label: section.label || "",
        options: Array.isArray(section.options) ? section.options : [],
      });
    }
  } else {
    const uncategorized = [];
    const byId = new Map();
    for (const option of options) {
      if (!option.section) {
        uncategorized.push(option);
        continue;
      }
      const id = typeof option.section === "string" ? option.section : option.section.id || option.section.label;
      const label = typeof option.section === "string" ? option.section : option.section.label || option.section.id;
      if (!byId.has(id)) {
        const section = { id: String(id), label: String(label || ""), options: [] };
        byId.set(id, section);
        grouped.push(section);
      }
      byId.get(id).options.push(option);
    }
    if (uncategorized.length || !grouped.length) grouped.unshift({ id: "", label: "", options: uncategorized });
  }
  const needle = query.trim().toLocaleLowerCase();
  return grouped.map(section => {
    const visibleOptions = !needle ? section.options : section.options.filter(option => {
      const text = [option.label, option.keywords, section.label].flat().filter(Boolean).join(" ").toLocaleLowerCase();
      return text.includes(needle);
    });
    return { ...section, options: visibleOptions, totalOptions: section.options.length };
  }).filter(section => section.options.length || (!needle && section.label));
}

function flattenSections(sections) {
  return sections.flatMap(section => section.options);
}

function clamp(value, min, max) {
  return Math.max(min, Math.min(value, max));
}

function placeMenu(triggerElement, menuElement, portalElement) {
  if (!triggerElement?.isConnected || !menuElement) return false;
  const trigger = triggerElement.getBoundingClientRect();
  if (!trigger.width && !trigger.height) return false;
  const constrainPortal = portalElement?.classList?.contains("confirm-dialog") || portalElement?.getAttribute?.("role") === "dialog";
  const frame = constrainPortal ? portalElement.getBoundingClientRect() : null;
  const margin = 8;
  const gap = 6;
  const viewportWidth = frame?.width ?? window.innerWidth;
  const viewportHeight = frame?.height ?? window.innerHeight;
  const viewportLeft = frame?.left ?? 0;
  const viewportTop = frame?.top ?? 0;
  const viewportRight = viewportLeft + viewportWidth;
  const viewportBottom = viewportTop + viewportHeight;
  const maxWidth = Math.max(180, Math.min(420, viewportWidth - margin * 2));
  const width = Math.min(Math.max(340, trigger.width), maxWidth);

  menuElement.style.width = `${width}px`;
  menuElement.style.maxWidth = `${maxWidth}px`;
  menuElement.style.maxHeight = "none";
  const naturalHeight = Math.max(menuElement.scrollHeight, 42);
  const below = viewportBottom - trigger.bottom - gap - margin;
  const above = trigger.top - viewportTop - gap - margin;
  const preferredBelow = Math.min(naturalHeight, 420);
  const openUp = false;
  const available = Math.max(below, 88);
  const maxHeight = Math.min(420, available);
  menuElement.style.maxHeight = `${maxHeight}px`;
  const height = Math.min(naturalHeight, maxHeight);

  const minLeft = viewportLeft + margin;
  const maxLeft = Math.max(minLeft, viewportRight - margin - width);
  const visualLeft = clamp(trigger.right - width, minLeft, maxLeft);
  const visualTop = openUp ? trigger.top - height - gap : trigger.bottom + gap;

  if (portalElement) {
    menuElement.style.position = "absolute";
    menuElement.style.left = `${visualLeft - (frame?.left ?? 0) + (portalElement.scrollLeft || 0)}px`;
    menuElement.style.top = `${visualTop - (frame?.top ?? 0) + (portalElement.scrollTop || 0)}px`;
    menuElement.style.right = "auto";
    menuElement.style.bottom = "auto";
  } else {
    menuElement.style.position = "fixed";
    menuElement.style.left = `${visualLeft}px`;
    menuElement.style.top = `${visualTop}px`;
    menuElement.style.right = "auto";
    menuElement.style.bottom = "auto";
  }
  return true;
}

function useMenuLifecycle({ disabled, inactive, trigger, menu, portalRef, onCloseFocus }) {
  const [state, setState] = useState("closed");
  const open = state === "opening" || state === "open";
  const portal = portalRef?.current || null;

  function close(restoreFocus = false) {
    if (!open) return;
    if (restoreFocus) {
      const target = trigger.current?.disabled ? onCloseFocus?.() : trigger.current;
      target?.focus?.({ preventScroll: true });
    }
    setState(reducedMotion() ? "closed" : "closing");
  }

  function openMenu() {
    if (!disabled && !inactive) flushSync(() => setState("opening"));
  }

  useLayoutEffect(() => {
    if (!open || disabled || !menu.current) return;
    if (!placeMenu(trigger.current, menu.current, portal)) {
      close();
    }
  });

  useLayoutEffect(() => {
    if (state === "opening") {
      if (reducedMotion()) {
        setState("open");
        return undefined;
      }
      const frame = requestAnimationFrame(() => setState("open"));
      return () => cancelAnimationFrame(frame);
    }
    if (state === "closing") {
      const timer = setTimeout(() => setState("closed"), 100);
      return () => clearTimeout(timer);
    }
    return undefined;
  }, [state]);

  useLayoutEffect(() => {
    if (disabled || inactive) close(menu.current?.contains(document.activeElement));
  }, [disabled, inactive, open]);

  useEffect(() => {
    if (!open) return undefined;
    const dismiss = event => {
      if (!trigger.current?.contains(event.target) && !menu.current?.contains(event.target) && !event.target.closest?.(`[data-selector-root="${menu.current?.dataset.selectorId}"]`)) close();
    };
    const reposition = event => {
      if (event.target instanceof Node && menu.current?.contains(event.target)) return;
      if (!placeMenu(trigger.current, menu.current, portal)) close(menu.current?.contains(document.activeElement));
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
  }, [open, disabled, portal]);

  useEffect(() => {
    if (!open || !menu.current || !window.BlobUI?.Scrollbar) return undefined;
    const scrollbar = window.BlobUI.Scrollbar.attach(menu.current);
    return () => scrollbar.destroy();
  }, [open]);

  return { state, setState, open, openMenu, close, portal };
}

function SelectorMenu({ menuRef, listId, label, multiple, state, sections, selected, onChoose, onKeyDown, emptyMessage, menuClassName = "", portalTarget, selectorId }) {
  const menu = <div ref={menuRef} id={listId} className={`kanban-select-menu selector-menu app-scrollbar ${menuClassName}`}
    role="listbox" aria-label={label} aria-multiselectable={multiple || undefined}
    data-state={state} data-selector-id={selectorId} hidden={state === "closed"} inert={state !== "opening" && state !== "open"}
    onKeyDown={event => { event.stopPropagation(); onKeyDown(event); }}>
    {sections.map((section, sectionIndex) => <React.Fragment key={section.id || sectionIndex}>
      {section.label && <div className="selector-menu-section" role="presentation">{section.label}</div>}
      {section.options.map(option => <button key={String(option.value)} type="button" className="kanban-select-option selector-option"
        role="option" tabIndex={-1} data-value={option.value} aria-label={option.label}
        aria-selected={selected.includes(option.value)} disabled={option.disabled === true} onClick={() => onChoose(option)}>
        {option.icon && <span className="selector-option-icon" aria-hidden="true"><SelectorOptionIcon icon={option.icon} /></span>}
        <span className="kanban-option-copy selector-option-copy"><span className="kanban-option-label selector-option-label">{option.label}</span></span>
        <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m3.5 8 3 3 6-6" /></svg>
      </button>)}
      {section.label && !section.options.length && <div className="selector-menu-empty-section" role="presentation">No matching options</div>}
    </React.Fragment>)}
    {!flattenSections(sections).length && emptyMessage && <div className="kanban-select-empty selector-menu-empty" role="status">{emptyMessage}</div>}
  </div>;
  return createPortal(menu, portalTarget || document.body);
}

function useSelectionKeyboard({ options, open, openMenu, close, menu, trigger }) {
  function focusOption(index) {
    const items = menu.current?.querySelectorAll('[role="option"]:not(:disabled)');
    if (!items?.length) {
      trigger.current?.focus({ preventScroll: true });
      return;
    }
    const item = items[(index + items.length) % items.length];
    item.focus({ preventScroll: true });
    const bounds = menu.current.getBoundingClientRect();
    const rect = item.getBoundingClientRect();
    if (rect.top < bounds.top + 8) menu.current.scrollTop -= bounds.top + 8 - rect.top;
    if (rect.bottom > bounds.bottom - 8) menu.current.scrollTop += rect.bottom - bounds.bottom + 8;
  }

  function handleKey(event) {
    const { key } = event;
    if (key === "Escape" && open) {
      event.preventDefault();
      close(true);
    } else if (key === "Tab" && open) {
      close(true);
    } else if (["ArrowDown", "ArrowUp", "Home", "End"].includes(key)) {
      event.preventDefault();
      const available = options.filter(option => !option.disabled);
      if (!open) {
        openMenu();
        if (key === "Home") window.setTimeout(() => focusOption(0), 0);
        if (key === "End") window.setTimeout(() => focusOption(available.length - 1), 0);
        return;
      }
      const index = available.findIndex(option => String(option.value) === document.activeElement?.dataset.value);
      if (key === "Home") focusOption(0);
      else if (key === "End") focusOption(available.length - 1);
      else focusOption(index < 0 ? key === "ArrowDown" ? 0 : available.length - 1 : index + (key === "ArrowDown" ? 1 : -1));
    }
  }
  return { focusOption, handleKey };
}

export function BasicSelector({
  id, rootId, menuId, label, labelId, valueId, descriptionId, options = [], sections: sectionInput, value, caption,
  onChange, multiple = false, selectionLimit = Infinity, closeOnSelect = !multiple, disabled = false, inactive = false, emptyMessage,
  icon, pressed, title, portalRef, fixed = false, rootClassName = "", menuClassName = "",
  triggerClassName = "", preservePointerFocus = false, restoreFocusOnSelect = true,
}) {
  const generatedId = useId().replace(/[^A-Za-z0-9_-]/g, "");
  const listId = menuId || `picker-${generatedId}`;
  const root = useRef(null);
  const trigger = useRef(null);
  const menu = useRef(null);
  const lifecycle = useMenuLifecycle({ disabled, inactive, trigger, menu, portalRef, onCloseFocus: () => document.getElementById("page-title") });
  const { state, open, openMenu, close, portal } = lifecycle;
  const selected = asArray(value, multiple);
  const normalizedSections = useMemo(() => normalizeSections(options, sectionInput), [options, sectionInput]);
  const flatOptions = useMemo(() => flattenSections(normalizedSections), [normalizedSections]);
  const selectedLabels = flatOptions.filter(option => selected.includes(option.value)).map(option => option.label);
  const display = caption ?? (multiple
    ? selected.length ? selectedLabels.join(", ").length <= 44 ? selectedLabels.join(", ") : `${selected.length} selected` : "None"
    : selectedLabels[0] ?? "");
  const keyboard = useSelectionKeyboard({ options: flatOptions, open, openMenu, close, menu, trigger });

  useLayoutEffect(() => {
    if (state === "opening") {
      const index = Math.max(0, flatOptions.findIndex(option => selected.includes(option.value)));
      keyboard.focusOption(index);
    }
  }, [state]);

  function choose(option) {
    let next;
    if (multiple) {
      next = selected.includes(option.value) ? selected.filter(item => item !== option.value) : [...selected, option.value];
      if (!selected.includes(option.value) && next.length > selectionLimit) return;
    } else {
      next = option.value;
    }
    const shouldClose = onChange?.(next) !== false && closeOnSelect;
    if (shouldClose) close(restoreFocusOnSelect);
  }

  function handleKey(event) {
    if (disabled || inactive) return;
    keyboard.handleKey(event);
  }

  return <div ref={root} id={rootId} className={`kanban-select selector-root ${rootClassName}`} data-selector-root={generatedId} onKeyDown={handleKey}>
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
    <SelectorMenu menuRef={menu} listId={listId} label={label} multiple={multiple} state={state} sections={normalizedSections}
      selected={selected} onChoose={choose} onKeyDown={handleKey} emptyMessage={emptyMessage} menuClassName={menuClassName}
      portalTarget={portal} selectorId={generatedId} />
    {fixed && <span className="sr-only">Fixed selector positioning</span>}
  </div>;
}

export function AdvancedSelector({
  id, rootId, menuId, label, options = [], sections: sectionInput, value, caption, onChange,
  multiple = false, selectionLimit = Infinity, closeOnSelect = !multiple, disabled = false, inactive = false,
  emptyMessage = "No matching options.", placeholder = "Search options", portalRef, rootClassName = "", menuClassName = "",
  inputAttributes = {}, restoreFocusOnSelect = true,
}) {
  const generatedId = useId().replace(/[^A-Za-z0-9_-]/g, "");
  const listId = menuId || `advanced-picker-${generatedId}`;
  const root = useRef(null);
  const trigger = useRef(null);
  const menu = useRef(null);
  const [query, setQuery] = useState("");
  const lifecycle = useMenuLifecycle({ disabled, inactive, trigger, menu, portalRef, onCloseFocus: () => document.getElementById("page-title") });
  const { state, open, openMenu, close, portal } = lifecycle;
  const selected = asArray(value, multiple);
  const allSections = useMemo(() => normalizeSections(options, sectionInput), [options, sectionInput]);
  const normalizedSections = useMemo(() => normalizeSections(options, sectionInput, query), [options, sectionInput, query]);
  const allOptions = useMemo(() => flattenSections(allSections), [allSections]);
  const flatOptions = useMemo(() => flattenSections(normalizedSections), [normalizedSections]);
  const selectedLabels = allOptions.filter(option => selected.includes(option.value)).map(option => option.label);
  const display = caption ?? (multiple
    ? selected.length ? selectedLabels.join(", ").length <= 44 ? selectedLabels.join(", ") : `${selected.length} selected` : "None"
    : selectedLabels[0] ?? "");
  const keyboard = useSelectionKeyboard({ options: flatOptions, open, openMenu, close, menu, trigger });

  useLayoutEffect(() => {
    if (!open && query) setQuery("");
  }, [open, query]);

  function choose(option) {
    let next;
    if (multiple) {
      next = selected.includes(option.value) ? selected.filter(item => item !== option.value) : [...selected, option.value];
      if (!selected.includes(option.value) && next.length > selectionLimit) return;
    } else {
      next = option.value;
    }
    const shouldClose = onChange?.(next) !== false && closeOnSelect;
    if (shouldClose) close(restoreFocusOnSelect);
  }

  function handleKey(event) {
    if (disabled || inactive) return;
    keyboard.handleKey(event);
  }

  return <div ref={root} id={rootId} className={`advanced-selector ${rootClassName}`} data-selector-root={generatedId} onKeyDown={handleKey}>
    <div className="advanced-selector-field">
      <input ref={trigger} id={id} type="text" role="combobox" aria-label={label} aria-expanded={open} aria-controls={listId}
        aria-haspopup="listbox" autoComplete="off" spellCheck={false} placeholder={placeholder} disabled={disabled}
        value={open ? query : display} readOnly={inactive}
        {...inputAttributes}
        onFocus={event => {
          inputAttributes.onFocus?.(event);
          openMenu();
          window.setTimeout(() => trigger.current?.select(), 0);
        }}
        onClick={event => {
          inputAttributes.onClick?.(event);
          openMenu();
        }}
        onInput={event => {
          inputAttributes.onInput?.(event);
          setQuery(event.currentTarget.value);
          if (!open) openMenu();
        }} />
      <svg viewBox="0 0 16 16" aria-hidden="true"><path d="m4 6 4 4 4-4" /></svg>
    </div>
    <SelectorMenu menuRef={menu} listId={listId} label={label} multiple={multiple} state={state} sections={normalizedSections}
      selected={selected} onChoose={choose} onKeyDown={handleKey} emptyMessage={emptyMessage} menuClassName={menuClassName}
      portalTarget={portal} selectorId={generatedId} />
  </div>;
}

export function SelectPicker(props) {
  return <BasicSelector {...props} />;
}
