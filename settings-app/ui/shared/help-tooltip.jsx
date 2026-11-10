import React, { useId } from "react";
import "../help-tooltip.css";

export function HelpTooltip({ label, text, size = "normal" }) {
  const id = useId().replace(/:/g, "");
  const tooltipId = `help-tooltip-${id}`;
  return <span className={`help-tooltip${size === "large" ? " help-tooltip-large" : ""}`}>
    <button type="button" className="help-tooltip-trigger" aria-label={label} aria-describedby={tooltipId}>
      <span aria-hidden="true">?</span>
    </button>
    <span id={tooltipId} className="help-tooltip-popover" role="tooltip">{text}</span>
  </span>;
}
