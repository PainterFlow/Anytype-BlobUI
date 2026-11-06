import React from "react";

export function Button({ type = "button", children, ...attributes }) {
  return <button type={type} {...attributes}>{children}</button>;
}

export function IconButton({ className = "neutral-icon-button", ...props }) {
  return <Button className={className} {...props} />;
}

export function DeleteIcon() {
  return <svg viewBox="0 0 20 20" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
    <path d="M8 4.5h9.5v11H8L3.5 10 8 4.5Z" /><path d="m11 8 4 4m0-4-4 4" />
  </svg>;
}

export function SettingsSection({ className = "settings-group", children, ...attributes }) {
  return <section className={className} {...attributes}>{children}</section>;
}

export function SettingsHeading({ id, title, description }) {
  return <div className="section-heading">
    <h2 id={id}>{title}</h2>
    {description != null && <p>{description}</p>}
  </div>;
}

export function SettingsGroup({ className = "settings-list", children, ...attributes }) {
  return <div className={className} {...attributes}>{children}</div>;
}

export function SettingsRow({ className = "settings-row", label, description, descriptionAttributes, children, ...attributes }) {
  return <div className={className} {...attributes}>
    {label != null && <div className="kanban-setting-copy">
      <span className="kanban-setting-title">{label}</span>
      {description != null && <span {...descriptionAttributes}>{description}</span>}
    </div>}
    {children}
  </div>;
}

export function Switch({ id, label, description, className = "native-accent-control settings-row", inputAttributes, checked, disabled, onChange }) {
  return <label className={className} htmlFor={id}>
    <span>{label}</span>
    <input id={id} type="checkbox" role="switch" aria-describedby={description?.id}
      {...inputAttributes} checked={checked} disabled={disabled}
      onChange={onChange && (event => onChange(event.currentTarget.checked, event))} />
    <span className="switch-track" aria-hidden="true" />
    {description && <span className="sr-only" id={description.id}>{description.text}</span>}
  </label>;
}

export function TextField({ label, inputAttributes }) {
  return <label className="input-field" htmlFor={inputAttributes.id}>
    <span>{label}</span>
    <input {...inputAttributes} />
  </label>;
}

export function VisibilityTextField({ label, inputAttributes, buttonAttributes }) {
  const fieldId = `${inputAttributes.id}-field`;
  return <div className="input-field advanced-row">
    <label htmlFor={inputAttributes.id}>{label}</label>
    <div id={fieldId} className="visibility-field" data-filled="false">
      <input {...inputAttributes} />
      <button type="button" {...buttonAttributes}>
        <svg className="visibility-icon" viewBox="0 0 20 20" aria-hidden="true">
          <path className="visibility-eye" d="M2 10s3-5 8-5 8 5 8 5-3 5-8 5-8-5-8-5Z" />
          <circle className="visibility-eye" cx="10" cy="10" r="2.25" />
          <path className="visibility-slash" d="m3 3 14 14" />
        </svg>
      </button>
    </div>
  </div>;
}
