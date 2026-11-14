import React from "react";
import { Button, CopyIcon, CopySuccessIcon } from "./settings-controls.jsx";
import "./copy-text-field.css";

export function CopyTextField({
  value,
  inputAttributes,
  copied = false,
  copyLabel = "Copy text",
  copiedLabel = "Copied",
  onCopy,
  className = "",
}) {
  const fieldClassName = ["copy-text-field", className].filter(Boolean).join(" ");

  return <div className={fieldClassName} data-copied={String(copied)}>
    <input {...inputAttributes} value={value} readOnly />
    <Button
      className="copy-text-field-button"
      aria-label={copied ? copiedLabel : copyLabel}
      title={copied ? copiedLabel : copyLabel}
      onClick={onCopy}
    >
      <CopyIcon />
      <CopySuccessIcon />
    </Button>
  </div>;
}
