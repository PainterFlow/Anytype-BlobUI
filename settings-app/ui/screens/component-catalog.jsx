import React, { useEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import "../modal.js";
import "../scrollbar.js";
import "../visibility-field.js";
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
import { AdvancedSelector, SelectPicker } from "../shared/select-picker.jsx";
import { Tabs } from "../shared/tabs.jsx";
import { FilterIcon, ReloadIcon, SearchIcon } from "../shared/catalog-icons.jsx";
import { HelpTooltip } from "../shared/help-tooltip.jsx";
import { CatalogList, CatalogListRow, CatalogListToolbar } from "../shared/catalog-list.jsx";
import { Sidebar } from "./shell.jsx";

const pickerOptions = [
  { value: "balanced", label: "Balanced" },
  { value: "spacious", label: "Spacious" },
  { value: "compact", label: "Compact" },
];

const sourceOptions = [
  { value: "types", label: "Object types", icon: "📄" },
  { value: "collections", label: "Collections", icon: "🗂️" },
  { value: "queries", label: "Queries", icon: "🔎" },
];

const listSections = [
  { id: "personal", label: "Personal space", icon: "🙂", items: [
    { id: "tasks", title: "Tasks", meta: "Private", icon: "✅" },
    { id: "notes", title: "Notes", meta: "Private" },
  ]},
  { id: "work", label: "Work space", icon: "💼", items: [
    { id: "projects", title: "Projects", meta: "Work", icon: "📁" },
  ]},
];

const typeSections = [
  { id: "personal", label: "Personal space", options: [
    { value: "personal-task", label: "Task", icon: "✅" },
    { value: "personal-note", label: "Note", icon: "📝" },
    { value: "personal-bookmark", label: "Bookmark", icon: "🔖" },
  ]},
  { id: "work", label: "Work space", options: [
    { value: "work-project", label: "Project", icon: "📁" },
    { value: "work-meeting", label: "Meeting", icon: "📅" },
    { value: "work-person", label: "Person", icon: "👤" },
  ]},
];

const searchExamples = [
  { title: "Buttons", description: "Neutral, accent, and icon actions" },
  { title: "Settings rows", description: "Labels, descriptions, and switches" },
  { title: "Text fields and pickers", description: "Text, secret, and select controls" },
  { title: "Tabs", description: "Keyboard selection and animated indicator" },
  { title: "Help tooltips", description: "Short hover and focus explanations" },
  { title: "Sidebar", description: "Navigation, Save, Undo, and Logs" },
];

async function closeCatalogWindow() {
  try {
    const current = window.__TAURI__?.window?.getCurrentWindow?.();
    if (current) {
      await current.close();
    } else {
      window.close();
    }
  } catch (error) {
    console.error("Could not close the component catalog.", error);
  }
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

function CatalogModalPreview() {
  const backdrop = useRef(null);
  const dialog = useRef(null);
  const trigger = useRef(null);
  const closeButton = useRef(null);
  const modal = useRef(null);

  useEffect(() => {
    if (!backdrop.current || !dialog.current) return undefined;
    const controller = window.BlobUI?.createModalController?.({
      backdrop: backdrop.current,
      dialog: dialog.current,
      initialFocus: () => closeButton.current,
      fallbackFocus: () => trigger.current,
      inertTargets: [document.querySelector(".component-catalog-page")].filter(Boolean),
    });
    modal.current = controller;
    return () => {
      controller?.destroy();
      modal.current = null;
    };
  }, []);

  const dialogContent = <div ref={backdrop} className="confirm-backdrop" hidden>
    <section ref={dialog} className="confirm-dialog component-catalog-dialog" role="dialog" aria-modal="true"
      aria-labelledby="catalog-dialog-title" aria-describedby="catalog-dialog-description" tabIndex={-1}>
      <Button ref={closeButton} className="confirm-close" aria-label="Close sample dialog" title="Close"
        onClick={() => modal.current?.close()}>
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="m2 2 8 8m0-8-8 8" /></svg>
      </Button>
      <h2 id="catalog-dialog-title">Apply sample change?</h2>
      <p id="catalog-dialog-description">This dialog uses the same focus, Escape, backdrop, and reduced-motion behavior as Settings.</p>
      <div className="confirm-actions">
        <Button className="confirm-cancel" onClick={() => modal.current?.close()}>Cancel</Button>
        <Button className="confirm-save" onClick={() => modal.current?.close()}>Apply</Button>
      </div>
    </section>
  </div>;

  return <>
    <Button className="confirm-cancel component-catalog-modal-trigger" ref={trigger}
      onClick={() => modal.current?.open()}>Open sample dialog</Button>
    {createPortal(dialogContent, document.body)}
  </>;
}

export function ComponentCatalog({ onClose }) {
  const [selectedTab, setSelectedTab] = useState(0);
  const [selectedStyle, setSelectedStyle] = useState("balanced");
  const [selectedSources, setSelectedSources] = useState(["types"]);
  const [selectedTypes, setSelectedTypes] = useState(["personal-task"]);
  const [enabled, setEnabled] = useState(true);
  const [search, setSearch] = useState("");
  const [listQuery, setListQuery] = useState("");
  const [listFilter, setListFilter] = useState([]);
  const [expanded, setExpanded] = useState(true);
  const [message, setMessage] = useState("Controls below use the same components as Settings.");
  const visibleListSections = listSections.map(section => ({ ...section, items: section.items.filter(item => {
    const matchesQuery = item.title.toLocaleLowerCase().includes(listQuery.trim().toLocaleLowerCase());
    const matchesFilter = !listFilter.length || (listFilter.includes("icons") && item.icon) || (listFilter.includes("no-icons") && !item.icon);
    return matchesQuery && matchesFilter;
  }) })).filter(section => section.items.length);
  const disclosureToggle = useRef(null);
  const scrollContainer = useRef(null);
  const pickerSurface = useRef(null);

  const requestClose = onClose ?? closeCatalogWindow;

  useEffect(() => {
    const closeOnEscape = event => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      if (event.target?.closest?.(".confirm-backdrop:not([hidden])")) return;
      event.preventDefault();
      requestClose();
    };
    document.addEventListener("keydown", closeOnEscape);
    return () => document.removeEventListener("keydown", closeOnEscape);
  }, [requestClose]);

  useEffect(() => {
    const visibility = window.VisibilityField?.bind({
      input: document.getElementById("catalog-secret"),
      button: document.getElementById("catalog-secret-toggle"),
      field: document.getElementById("catalog-secret-field"),
      showLabel: "Show sample value",
      hideLabel: "Hide sample value",
      defaultVisible: false,
    });
    return () => visibility?.destroy();
  }, []);

  useEffect(() => {
    const scrollbar = scrollContainer.current && window.BlobUI?.Scrollbar?.attach(scrollContainer.current, ".component-catalog-content");
    return () => scrollbar?.destroy();
  }, []);

  const visibleSearchExamples = searchExamples.filter(example =>
    `${example.title} ${example.description}`.toLocaleLowerCase().includes(search.trim().toLocaleLowerCase()));

  return <main className="component-catalog-page" aria-labelledby="component-catalog-title">
    <header className="component-catalog-titlebar" data-tauri-drag-region>
      <span className="component-catalog-window-title" data-tauri-drag-region>BlobUI Settings</span>
      <Button className="window-control window-close component-catalog-close" aria-label="Close component catalog"
        title="Close component catalog" onClick={requestClose}>
        <svg viewBox="0 0 12 12" aria-hidden="true"><path d="m2 2 8 8m0-8-8 8" /></svg>
      </Button>
    </header>
    <div ref={scrollContainer} className="component-catalog-scroll app-scrollbar">
      <div className="component-catalog-content">
        <header className="component-catalog-heading">
          <h1 id="component-catalog-title">Component catalog</h1>
          <p>Interactive examples of the shared controls used throughout Settings.</p>
        </header>

        <div className="component-catalog-grid">
          <CatalogSection title="Buttons and icon controls" description="Neutral and accent actions, disabled state, and toolbar icons.">
            <SettingsGroup className="settings-list component-catalog-button-list">
              <div className="component-catalog-button-row">
                <Button className="confirm-cancel" onClick={() => setMessage("The neutral button works.")}>Neutral action</Button>
                <Button className="confirm-save" onClick={() => setMessage("The accent button works.")}>Save sample</Button>
                <Button className="confirm-cancel" disabled>Unavailable</Button>
                <IconButton aria-label="Remove sample item" title="Remove sample item" onClick={() => setMessage("The icon button works.")}>
                  <DeleteIcon />
                </IconButton>
              </div>
              <div className="component-catalog-icon-row" role="group" aria-label="Toolbar icon controls">
                <IconButton aria-label="Search examples" title="Search examples" onClick={() => setMessage("Search toolbar control selected.")}><SearchIcon /></IconButton>
                <IconButton aria-label="Filter examples" title="Filter examples" onClick={() => setMessage("Filter toolbar control selected.")}><FilterIcon /></IconButton>
                <IconButton aria-label="Reload examples" title="Reload examples" onClick={() => setMessage("Reload toolbar control selected.")}><ReloadIcon /></IconButton>
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

          <CatalogSection title="Text fields and pickers" description="Editable text, secret visibility, selectors, sections, icons, and search.">
            <div ref={pickerSurface} className="component-catalog-picker-surface">
              <SettingsGroup className="settings-list component-catalog-fields">
                <TextField label="Interface font" inputAttributes={{ id: "catalog-font", defaultValue: "Inter" }} />
                <VisibilityTextField label="Sample secret" inputAttributes={{ id: "catalog-secret", type: "password", defaultValue: "sample-key-1234" }}
                  buttonAttributes={{ id: "catalog-secret-toggle", type: "button", className: "visibility-toggle", "aria-pressed": "false", "aria-label": "Show sample value", title: "Show sample value" }} />
                <div className="component-catalog-picker-row">
                  <span id="catalog-style-label">Board layout</span>
                  <SelectPicker id="catalog-style" label="Board layout" labelId="catalog-style-label" valueId="catalog-style-value"
                    portalRef={pickerSurface} options={pickerOptions} value={selectedStyle} onChange={setSelectedStyle} />
                </div>
                <div className="component-catalog-picker-row">
                  <span id="catalog-sources-label">Source categories</span>
                  <SelectPicker id="catalog-sources" label="Source categories" labelId="catalog-sources-label" valueId="catalog-sources-value"
                    portalRef={pickerSurface} options={sourceOptions} value={selectedSources} multiple selectionLimit={2} closeOnSelect={false}
                    caption={`${selectedSources.length} selected`} onChange={setSelectedSources} />
                </div>
                <div className="component-catalog-picker-row">
                  <span id="catalog-types-label">Filtered object types</span>
                  <AdvancedSelector id="catalog-types" label="Filtered object types" sections={typeSections}
                    portalRef={pickerSurface} value={selectedTypes} multiple closeOnSelect={false}
                    caption={selectedTypes.length ? `${selectedTypes.length} selected` : "None"}
                    placeholder="Search object types" onChange={setSelectedTypes} />
                </div>
              </SettingsGroup>
            </div>
          </CatalogSection>

          <CatalogSection title="Catalog lists" description="Grouped rows support space and item icons, secondary text, and keyboard-safe hover styling.">
            <CatalogListToolbar prefix="catalog-list-demo" searchLabel="Search catalog rows" query={listQuery} onQueryChange={setListQuery}
              reloadLabel="Reload catalog rows" onReload={() => {}} filterLabel="Filter catalog rows" filterMultiple filterCloseOnSelect={false}
              filterValue={listFilter} onFilterChange={setListFilter} filterOptions={[{ value: "icons", label: "With icons" }, { value: "no-icons", label: "Without icons" }]} />
            <CatalogList sections={visibleListSections} emptyMessage="No catalog rows match." renderItem={item => <CatalogListRow key={item.id} icon={item.icon} title={item.title}
              meta={item.meta} onClick={() => setMessage(`${item.title} row selected.`)} />} />
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
              <span className="component-catalog-search-result">{search ? `${visibleSearchExamples.length} matches` : "Search the examples"}</span>
            </div>
            <ul className="component-catalog-search-results" aria-label="Searchable examples">
              {visibleSearchExamples.map(example => <li key={example.title}><Button className="component-catalog-search-result-item"
                onClick={() => setMessage(`${example.title} example selected.`)}>
                <span>{example.title}</span><small>{example.description}</small>
              </Button></li>)}
              {!visibleSearchExamples.length && <li className="component-catalog-search-empty" role="status">No examples match this search.</li>}
            </ul>
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

          <CatalogSection title="Dialogs" description="Shared confirmation styling, focus handling, Escape, and backdrop dismissal.">
            <div className="component-catalog-dialog-preview"><CatalogModalPreview /></div>
          </CatalogSection>

          <CatalogSection title="Help tooltips" description="Short help appears on hover or keyboard focus.">
            <div className="component-catalog-help-preview">
              <span className="kanban-setting-title">Color by property</span>
              <HelpTooltip label="About color rules" text="Rules use the optional Anytype Local API v2 connection." />
              <HelpTooltip variant="warning" label="Experimental option warning" text="Experimental option. Expect interface bugs, but it should cause no data loss." />
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
