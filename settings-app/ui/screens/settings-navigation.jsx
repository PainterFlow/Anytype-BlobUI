import React, { useRef, useState } from "react";
import { DisclosurePanel } from "../shared/disclosure.jsx";
import { Button } from "../shared/settings-controls.jsx";
import { preferenceLabels } from "./preferences-accessibility.jsx";

const icons = {
  Colors: null,
  Font: <path d="M3 4h14M10 4v13m-4 0h8M3 4v3m14-3v3" />,
  "Space banner": <><rect x="2" y="4" width="16" height="12" rx="2" /><circle cx="7" cy="8" r="1" /><path d="m3 15 5-5 3 3 3-4 4 6" /></>,
  Kanban: <><rect x="2" y="3" width="4" height="14" rx="1" /><rect x="8" y="3" width="4" height="9" rx="1" /><rect x="14" y="3" width="4" height="12" rx="1" /></>,
  Calendar: <><rect x="3" y="4" width="14" height="13" rx="2" /><path d="M6 2v4m8-4v4M3 8h14M6 11h2m4 0h2m-8 3h2" /></>,
  Appearance: <><path d="M3 5h14M3 10h14M3 15h14" /><circle cx="8" cy="5" r="1.5" fill="var(--sidebar)" /><circle cx="13" cy="10" r="1.5" fill="var(--sidebar)" /><circle cx="6" cy="15" r="1.5" fill="var(--sidebar)" /></>,
  "Color by property": <><path d="M10 2a8 8 0 1 0 0 16h1.2a1.8 1.8 0 0 0 1.3-3c-.6-.7-.1-1.9.9-1.9H15a3 3 0 0 0 3-3A8 8 0 0 0 10 2Z" /><circle cx="6.5" cy="8" r="1" fill="currentColor" /><circle cx="10" cy="5.5" r="1" fill="currentColor" /><circle cx="13.5" cy="8" r="1" fill="currentColor" /></>,
  "Kanban Calculations": <><rect x="3" y="2.5" width="14" height="15" rx="2" /><path d="M6 6h8M6 9.5h2m4 0h2M6 13h2m4 0h2" /></>,
  "API key": <><circle cx="7" cy="7" r="4" /><path d="m10 10 7 7m-3-3 2-2m-4 0 2-2" /></>,
  Experiments: <><path d="M7 2.5h6M8 2.5v5.2l-4.3 7.1A2 2 0 0 0 5.4 18h9.2a2 2 0 0 0 1.7-3.2L12 7.7V2.5M6 13h8" /></>,
  "App Settings": <><path d="M3 5h14M3 10h14M3 15h14" /><circle cx="7" cy="5" r="2" fill="var(--sidebar)" /><circle cx="13" cy="10" r="2" fill="var(--sidebar)" /><circle cx="8" cy="15" r="2" fill="var(--sidebar)" /></>,
  Settings: <><path d="M3 5h14M3 10h14M3 15h14" /><circle cx="7" cy="5" r="2" /><circle cx="13" cy="10" r="2" /></>,
  Permissions: <><path d="m10 2 7 3v5c0 4-7 8-7 8s-7-4-7-8V5l7-3Z" /><path d="m6 10 3 3 5-6" /></>,
  Projects: <path d="M2 5h6l2 2h8v10H2Z" />,
};

function NavigationGroup({ id, label, sections, active, expanded, onNavigate, onSection, children, hidden }) {
  const toggle = useRef(null);
  const panelId = id + "-section-links";
  return <>
    <Button ref={toggle} id={id + "-nav"} className={"nav-item" + (active ? " active" : "")}
      aria-current={active ? "page" : "false"} data-settings-page={id} hidden={hidden}
      aria-controls={sections ? panelId : undefined} aria-expanded={sections ? expanded : undefined}
      data-disclosure-toggle={sections ? "" : undefined} onClick={() => onNavigate(id)}>
      {children}<span>{label}</span>
    </Button>
    {sections && <DisclosurePanel id={panelId} toggle={toggle} expanded={expanded}
      className="documentation-children v21-section-links">
      {sections.map((section, index) =>
        <Button key={section} className="documentation-item documentation-child v21-section-link"
          onClick={() => onSection(id, index)}>
          <span className="documentation-item-icon">{section === "Colors"
            ? <span className="settings-emoji-icon" aria-hidden="true">🎨</span>
            : <svg viewBox="0 0 20 20" aria-hidden="true">{icons[section]}</svg>}</span>
          <span className="documentation-item-label">{section}</span>
        </Button>
      )}
    </DisclosurePanel>}
  </>;
}

export function SettingsNavigation({ page, onNavigate, onSection }) {
  const [expanded, setExpanded] = useState(null);
  const navigate = id => {
    setExpanded(current => current === id || id === "accessibility" ? null : id);
    onNavigate(id);
  };
  const section = (id, index) => {
    setExpanded(id);
    onSection(id, index);
  };
  const group = id => ({ id, active: page === id, expanded: expanded === id, onNavigate: navigate, onSection: section });
  return <nav id="sidebar-nav" className="sidebar-nav" aria-label="Settings">
    <span className="nav-group-label">Application</span>
    <NavigationGroup {...group("preferences")} label="Preferences" sections={preferenceLabels}>
      <svg className="nav-icon" viewBox="0 0 20 20" aria-hidden="true"><circle cx="7" cy="10" r="5" fill="currentColor" /><path d="M12 5a5 5 0 0 1 0 10 7 7 0 0 0 0-10" fill="currentColor" /></svg>
    </NavigationGroup>
    <NavigationGroup {...group("accessibility")} label="Accessibility">
      <svg className="nav-icon" viewBox="0 0 20 20" aria-hidden="true"><circle cx="10" cy="10" r="7" fill="none" stroke="currentColor" strokeWidth="1.5" /><circle cx="10" cy="6" r="1.2" fill="currentColor" /><path d="M6.5 8.5h7M10 8.5v6m0-3-3 3m3-3 3 3" fill="none" stroke="currentColor" strokeWidth="1.4" strokeLinecap="round" strokeLinejoin="round" /></svg>
    </NavigationGroup>
    <NavigationGroup {...group("calendar-events")} label="Calendar Events" sections={["Appearance", "Color by property"]}>
      <svg className="nav-icon" viewBox="0 0 20 20" aria-hidden="true"><rect x="3" y="4" width="14" height="13" rx="2" /><path d="M6 2v4m8-4v4M3 8h14M6 11h2m4 0h2m-8 3h2" /></svg>
    </NavigationGroup>
    <NavigationGroup {...group("kanban-views")} label="Kanban Views" sections={["Appearance", "Kanban Calculations"]}>
      <svg className="nav-icon" viewBox="0 0 20 20" aria-hidden="true"><rect x="2" y="3" width="4" height="14" rx="1" /><rect x="8" y="3" width="4" height="9" rx="1" /><rect x="14" y="3" width="4" height="12" rx="1" /></svg>
    </NavigationGroup>
    <NavigationGroup {...group("advanced")} label="Advanced Settings" sections={["API key", "App Settings", "Experiments"]}>
      <svg className="nav-icon" viewBox="0 0 20 20" aria-hidden="true"><path d="M3 5h14M3 10h14M3 15h14" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" /><circle cx="7" cy="5" r="2" fill="var(--sidebar)" stroke="currentColor" strokeWidth="1.5" /><circle cx="13" cy="10" r="2" fill="var(--sidebar)" stroke="currentColor" strokeWidth="1.5" /><circle cx="8" cy="15" r="2" fill="var(--sidebar)" stroke="currentColor" strokeWidth="1.5" /></svg>
    </NavigationGroup>
    <span className="nav-group-label">Integrations</span>
    <NavigationGroup {...group("codex")} label="Anyblob" sections={["Settings", "Permissions", "Projects"]}>
      <span className="nav-icon"><svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" strokeWidth="1.5" strokeLinecap="round" strokeLinejoin="round">
        {Array.from({ length: 6 }, (_, index) => <path key={index} transform={`rotate(${index * 60} 12 12)`} d="M8.5 13V6l5-2.9a5.3 5.3 0 0 1 7.2 1.9" />)}
      </svg></span>
    </NavigationGroup>
  </nav>;
}
