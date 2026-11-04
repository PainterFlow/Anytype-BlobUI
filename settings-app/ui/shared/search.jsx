import React, { useLayoutEffect, useRef, useState } from "react";
import { IconButton } from "./settings-controls.jsx";

export function ExpandableSearch({ id, value, onChange, label, icon, placeholder = label, toggleId = `${id}-toggle`, disabled }) {
  const [expanded, setExpanded] = useState(false);
  const button = useRef(null);
  const input = useRef(null);
  const focusInput = useRef(false);
  const visible = expanded || Boolean(value);

  useLayoutEffect(() => {
    if (visible && focusInput.current) {
      focusInput.current = false;
      input.current?.focus();
    }
  }, [visible]);

  function open() {
    if (visible) input.current?.focus();
    else {
      focusInput.current = true;
      setExpanded(true);
    }
  }

  return <div id={`${id}-control`} className="expandable-search" data-expanded={visible}
    onBlur={event => {
      if (!event.currentTarget.contains(event.relatedTarget) && !value) setExpanded(false);
    }}>
    <IconButton ref={button} id={toggleId} aria-label={label} aria-controls={id} disabled={disabled}
      aria-expanded={visible} onClick={open}>{icon}</IconButton>
    <label className="sr-only" htmlFor={id}>{label}</label>
    <input ref={input} id={id} type="search" disabled={disabled}
      tabIndex={visible ? 0 : -1} placeholder={placeholder} value={value}
      onChange={event => onChange(event.currentTarget.value)}
      onKeyDown={event => {
        if (event.key !== "Escape") return;
        event.preventDefault();
        onChange("");
        setExpanded(false);
        button.current?.focus();
      }} />
  </div>;
}
