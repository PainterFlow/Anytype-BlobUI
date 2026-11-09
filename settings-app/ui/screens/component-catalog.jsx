import React, { useEffect, useRef, useState } from "react";
import {
  Button,
  DeleteIcon,
  IconButton,
  SettingsGroup,
  SettingsHeading,
  SettingsRow,
  SettingsSection,
  Switch,
  TextField,
  VisibilityTextField,
} from "../shared/settings-controls.jsx";
import { DisclosurePanel } from "../shared/disclosure.jsx";
import { ExpandableSearch } from "../shared/search.jsx";
import { SelectPicker } from "../shared/select-picker.jsx";
import { Tabs } from "../shared/tabs.jsx";
import { Sidebar } from "./shell.jsx";

const pickerOptions = [
  { value: "balanced", label: "Balanced" },
  { value: "spacious", label: "Spacious" },
  { value: "compact", label: "Compact" },
];

function closeCurrentWindow() {
  try {
    const current = window.__TAURI__?.window?.getCurrentWindow?.();
    if (current && typeof current.close === "function") {
      Promise.resolve(current.close()).catch(() => window.close());
      return;
    }
  } catch (_) {
    // The browser fallback below keeps the preview usable outside Tauri.
  }
  window.close();
}

function CatalogSection({ title, description, children, className = "" }) {
  return <SettingsSection className={`settings-group component-catalog-card ${className}`}>
    <SettingsHeading title={title} description={description} />
    {children}
  </SettingsSection>;
}

function SidebarPreview({ onMessage }) {
  const [selected, setSelected] = useState("Preferences");
  const rows = ["Preferences", "Accessibility", "Calendar Events", "Kanban Views", "Advanced Settings"];
  const navigation = <nav className="sidebar-nav" aria-label="Catalog preview navigation">
    <div className="nav-group-label">Settings</div>
    {rows.map((label, index) => <Button key={label} className={`nav-item${selected === label ? " active" : ""}`}
      aria-current={selected === label ? "page" : undefined} onClick={() => setSelected(label)}>
      <svg className="nav-icon" viewBox="0 0 20 20" aria-hidden="true">
        {index === 0 ? <><circle cx="10" cy="10" r="6.5" /><path d="M10 6.5v7M6.5 10h7" /></>
          : <><rect x="3.5" y="3.5" width="13" height="13" rx="2" /><path d="M7 7h6M7 10h6M7 13h4" /></>}
      </svg>
      <span>{label}</span>
    </Button>)}
  </nav>;

  return <div className="component-catalog-shell-preview">
    <Sidebar navigation={navigation} saveVisible saveDisabled={false} discardDisabled={false} saveBusy={false}
      onSave={() => onMessage("The sample Save button works.")}
      onDiscard={() => onMessage("The sample Undo button works.")}
      logRows={[{ text: "Sample log entry", level: "info" }]} logSeverity="info" />
    <div className="component-catalog-shell-content" aria-live="polite">
      <span className="component-catalog-shell-label">{selected}</span>
      <span>Application content preview</span>
    </div>
  </div>;
}

export function ComponentCatalog({ onClose }) {
  const [leaving, setLeaving] = useState(false);
  const [selectedTab, setSelectedTab] = useState(0);
  const [selectedStyle, setSelectedStyle] = useState("balanced");
  const [enabled, setEnabled] = useState(true);
  const [search, setSearch] = useState("");
  const [expanded, setExpanded] = useState(true);
  const [message, setMessage] = useState("Controls below use the same components as Settings.");
  const closeTimer = useRef(null);
  const closeStarted = useRef(false);
  const disclosureToggle = useRef(null);

  function requestClose() {
    if (closeStarted.current) return;
    closeStarted.current = true;
    setLeaving(true);
    const reducedMotion = document.documentElement.dataset.reducedMotion === "true"
      || window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
    closeTimer.current = window.setTimeout(() => {
      if (onClose) onClose();
      else closeCurrentWindow();
    }, reducedMotion ? 0 : 320);
  }

  useEffect(() => {
    const closeOnEscape = event => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      event.preventDefault();
      requestClose();
    };
    document.addEventListener("keydown", closeOnEscape);
    return () => {
      document.removeEventListener("keydown", closeOnEscape);
      if (closeTimer.current !== null) window.clearTimeout(closeTimer.current);
    };
  }, [onClose]);

  useEffect(() => {
    const visibility = window.BlobUI?.VisibilityField?.bind({
      input: document.getElementById("catalog-secret"),
      button: document.getElementById("catalog-secret-toggle"),
      field: document.getElementById("catalog-secret-field"),
      showLabel: "Show sample value",
      hideLabel: "Hide sample value",
      defaultVisible: false,
    });
    return () => visibility?.destroy();
  }, []);

  return <main className={`component-catalog-page${leaving ? " leaving" : ""}`} aria-labelledby="component-catalog-title">
    <header className="component-catalog-titlebar" data-tauri-drag-region>
      <span className="component-catalog-window-title" data-tauri-drag-region>BlobUI Settings</span>
      <Button className="window-control window-close component-catalog-close" aria-label="Close component catalog"
        title="Close component catalog" onClick={requestClose}>
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="m2 2 8 8m0-8-8 8" /></svg>
      </Button>
    </header>
    <div className="component-catalog-scroll app-scrollbar">
      <div className="component-catalog-content">
        <header className="component-catalog-heading">
          <h1 id="component-catalog-title">Component catalog</h1>
          <p>Interactive examples of the shared controls used throughout Settings.</p>
        </header>

        <div className="component-catalog-grid">
          <CatalogSection title="Buttons" description="Shared buttons with the current Settings styles.">
            <SettingsGroup className="settings-list component-catalog-button-list">
              <div className="component-catalog-button-row">
                <Button className="confirm-cancel" onClick={() => setMessage("The neutral button works.")}>Neutral action</Button>
                <Button className="confirm-save" onClick={() => setMessage("The accent button works.")}>Save sample</Button>
                <IconButton aria-label="Remove sample item" title="Remove sample item" onClick={() => setMessage("The icon button works.")}>
                  <DeleteIcon />
                </IconButton>
              </div>
              <p className="component-catalog-feedback" role="status">{message}</p>
            </SettingsGroup>
          </CatalogSection>

          <CatalogSection title="Settings rows and switches" description="Labels, descriptions, grouped rows, and toggles.">
            <SettingsGroup className="settings-list">
              <SettingsRow label="Object hover" description="Show a preview when the pointer rests on an object." />
              <Switch id="catalog-enabled" label="Enable sample setting" checked={enabled}
                onChange={setEnabled} inputAttributes={{ "aria-describedby": "catalog-enabled-description" }}
                description={{ id: "catalog-enabled-description", text: "This switch is interactive." }} />
              <Switch id="catalog-disabled" label="Unavailable setting" checked={false} disabled
                inputAttributes={{ "aria-label": "Unavailable setting" }} />
            </SettingsGroup>
          </CatalogSection>

          <CatalogSection title="Text fields and pickers" description="Reusable text, secret, and select controls.">
            <SettingsGroup className="settings-list component-catalog-fields">
              <TextField label="Interface font" inputAttributes={{ id: "catalog-font", defaultValue: "Inter" }} />
              <VisibilityTextField label="Sample secret" inputAttributes={{ id: "catalog-secret", type: "password", defaultValue: "sample-key-1234" }}
                buttonAttributes={{ id: "catalog-secret-toggle", type: "button", className: "visibility-toggle", "aria-pressed": "false", "aria-label": "Show sample value", title: "Show sample value" }} />
              <div className="component-catalog-picker-row">
                <span id="catalog-style-label">Board layout</span>
                <SelectPicker id="catalog-style" label="Board layout" labelId="catalog-style-label" valueId="catalog-style-value"
                  options={pickerOptions} value={selectedStyle} onChange={setSelectedStyle} />
              </div>
            </SettingsGroup>
          </CatalogSection>

          <CatalogSection title="Tabs" description="Keyboard navigation, selection, and the animated indicator.">
            <Tabs id="catalog-tabs" label="Component examples" selected={selectedTab} onSelect={setSelectedTab}
              items={[
                { tabId: "catalog-tab-controls", panelId: "catalog-panel-controls", title: "Controls" },
                { tabId: "catalog-tab-layout", panelId: "catalog-panel-layout", title: "Layout" },
                { tabId: "catalog-tab-motion", panelId: "catalog-panel-motion", title: "Motion" },
              ]}>
              <p>Buttons, fields, and pickers share the same React controls.</p>
              <p>Settings rows keep their spacing and labels aligned.</p>
              <p>Transitions follow the reduced-motion preference.</p>
            </Tabs>
          </CatalogSection>

          <CatalogSection title="Search and disclosure" description="Expanding search and the Docs-style disclosure panel.">
            <div className="component-catalog-search-row">
              <ExpandableSearch id="catalog-search" label="Search examples" value={search} onChange={setSearch} />
              <span className="component-catalog-search-result">{search ? `Filtering for ${search}` : "Search examples"}</span>
            </div>
            <div className="component-catalog-disclosure">
              <Button ref={disclosureToggle} className="documentation-item component-catalog-disclosure-toggle"
                aria-expanded={expanded} aria-controls="catalog-disclosure-panel" onClick={() => setExpanded(value => !value)}>
                <span className="documentation-item-label">Reusable panel content</span>
                <svg className="documentation-chevron" viewBox="0 0 16 16" aria-hidden="true"><path d="m5 3 5 5-5 5" /></svg>
              </Button>
              <DisclosurePanel id="catalog-disclosure-panel" expanded={expanded} toggle={disclosureToggle}>
                <Button className="documentation-item documentation-child" onClick={() => setMessage("The disclosure child button works.")}>Example child action</Button>
              </DisclosurePanel>
            </div>
          </CatalogSection>

          <CatalogSection title="Sidebar" description="The production Settings sidebar with sample navigation and actions." className="component-catalog-wide-card">
            <SidebarPreview onMessage={setMessage} />
          </CatalogSection>
        </div>
      </div>
    </div>
  </main>;
}
