import React, { useEffect, useState } from "react";
import { FormulaPageView } from "../formula-page.jsx";
import { SelectPicker } from "../shared/select-picker.jsx";
import {
  Button,
  SettingsGroup,
  SettingsHeading,
  SettingsRow,
  SettingsSection,
  Switch,
  TextField,
} from "../shared/settings-controls.jsx";
import { Tabs } from "../shared/tabs.jsx";
import { CalendarColors } from "./calendar-colors.jsx";

export const preferenceLabels = ["Colors", "Font", "Space Banner"];

function tabItems(prefix, labels) {
  return labels.map((title, index) => ({
    title, tabId: `${prefix}-tab-${index}`, panelId: `${prefix}-panel-${index}`,
  }));
}

const preferenceTabs = tabItems("preference", preferenceLabels);
const calendarTabs = tabItems("calendar-events", ["Appearance", "Color by property"]);
const kanbanTabs = tabItems("kanban-views", ["Appearance", "Kanban Calculations"]);

function hasValue(values, key) {
  return Object.prototype.hasOwnProperty.call(values, key);
}

function settingSwitch(values, onChange, id, label, className, description, inputAttributes) {
  const props = {
    id,
    label,
    className,
    description,
    inputAttributes,
    onChange: onChange && ((value, event) => onChange(id, value, event)),
  };
  if (hasValue(values, id)) {
    if (onChange) {
      const { defaultChecked, ...attributes } = inputAttributes || {};
      props.inputAttributes = attributes;
      props.checked = values[id];
    }
    else props.inputAttributes = { ...inputAttributes, defaultChecked: values[id] };
  }
  if (inputAttributes && "disabled" in inputAttributes) props.disabled = Boolean(inputAttributes.disabled);
  return <Switch {...props} />;
}

function ColorPlane({ id = "color-plane", cursorId = "color-cursor", saturationId = "accent-saturation", brightnessId = "accent-brightness", label = "Color saturation and brightness" }) {
  return <div className="color-plane" id={id} role="group" aria-label={label}>
    <span className="color-cursor" id={cursorId} aria-hidden="true" />
    <input className="sr-only" id={saturationId} type="range" min="0" max="100" defaultValue="64" aria-label="Color saturation" />
    <input className="sr-only" id={brightnessId} type="range" min="0" max="100" defaultValue="87" aria-label="Color brightness" />
  </div>;
}

const basedThemeOptions = [
  { value: "auto", label: "Auto", description: "Choose a light or dark palette from the main color." },
  { value: "light", label: "Light Mode" },
  { value: "dark", label: "Dark Mode" },
  { value: "inherit", label: "Don't Override", description: "Use the Light or Dark mode selected in Anytype." },
];

function CustomThemes({ values, onChange, themes = [], directory = "", disabled = false }) {
  const selectedTheme = values["custom-theme"] || "";
  const hasSelectedTheme = !selectedTheme || themes.some(theme => theme.id === selectedTheme);
  const options = [
    { value: "", label: "None", description: "Use the generated palette without a preset." },
    ...themes.map(theme => ({ value: theme.id, label: theme.name, description: theme.author })),
    ...(!hasSelectedTheme ? [{ value: selectedTheme, label: `Missing theme (${selectedTheme})`, description: "The selected CSS file is no longer available." }] : []),
  ];
  const inputValue = (id, fallback) => {
    if (!Object.prototype.hasOwnProperty.call(values, id)) return fallback;
    return { ...fallback, value: values[id], onChange: event => onChange?.(id, event.currentTarget.value, event) };
  };

  return <SettingsSection className="settings-group custom-themes-group" aria-labelledby="custom-themes-heading">
    <SettingsHeading id="custom-themes-heading" title="Custom Themes" description="Build an Anytype color palette from a main color and optional CSS preset." />
    <SettingsGroup className="settings-list custom-theme-list">
      <SettingsRow className="kanban-setting-row custom-theme-select-row" label="Based Theme">
        <SelectPicker id="based-theme" rootId="based-theme-select" menuId="based-theme-options"
          label="Based Theme" value={values["based-theme"] || "auto"} options={basedThemeOptions} disabled={disabled}
          onChange={value => onChange?.("based-theme", value)} />
      </SettingsRow>
      <SettingsRow className="kanban-setting-row custom-theme-select-row" label="Preset Themes">
        <SelectPicker id="custom-theme" rootId="custom-theme-select" menuId="custom-theme-options"
          label="Preset Themes" value={selectedTheme} options={options} disabled={disabled}
          emptyMessage="No theme files were found." onChange={value => onChange?.("custom-theme", value)} />
      </SettingsRow>
      <div className="custom-theme-color-row">
        <div className="kanban-setting-copy"><span className="kanban-setting-title">Main color</span></div>
        <div id="theme-color-editor" className="color-editor custom-theme-color-editor">
          <ColorPlane id="theme-color-plane" cursorId="theme-color-cursor" saturationId="theme-color-saturation" brightnessId="theme-color-brightness" label="Theme color saturation and brightness" />
          <div className="color-controls">
            <label className="hue-field" htmlFor="theme-color-hue">Hue</label>
            <input id="theme-color-hue" className="hue-slider" type="range" min="0" max="359" defaultValue="12" aria-label="Theme color hue" />
            <label className="color-value-label" htmlFor="theme-color-value">Hex color</label>
            <div className="color-value-row">
              <span id="theme-color-swatch" className="accent-swatch" aria-hidden="true" />
              <input id="theme-color-value" className="color-text-input" type="text" spellCheck="false" autoComplete="off"
                aria-describedby="theme-color-error" {...inputValue("theme-color-value", {})} />
            </div>
          </div>
        </div>
        <p className="field-error" id="theme-color-error" aria-live="polite" />
      </div>
    </SettingsGroup>
    <p className="custom-theme-help">Add a CSS file with a commented name and author header to <code>{directory || "Settings/themes/built-in-themes"}</code>. Press Ctrl+R in BlobUI Settings to reload the theme list.</p>
  </SettingsSection>;
}

const boardLayouts = [
  { value: "default", label: "Default" },
  { value: "balanced", label: "Balanced" },
  { value: "compact", label: "Compact" },
];
const formulaLayouts = [
  { value: "balanced", label: "Balanced" },
  { value: "spacious", label: "Spacious" },
];

function SpaceBannerPreview() {
  return <div className="space-sidebar-preview" inert role="img" aria-label="Static preview of the Anytype sidebar with a space banner">
    <div className="space-preview-banner">
      <img id="space-preview-image" className="space-preview-image" alt="" />
      <div className="space-preview-shade" aria-hidden="true" />
      <span className="space-preview-logo" aria-hidden="true"><svg viewBox="0 0 78 78"><defs><linearGradient id="space-mascot" x2="0" y2="1"><stop stopColor="#8494ed" /><stop offset="1" stopColor="#b34cdb" /></linearGradient></defs><rect width="78" height="78" rx="20" fill="url(#space-mascot)" /><g fill="#ded4ff"><circle cx="20" cy="23" r="9.5" /><circle cx="58" cy="23" r="9.5" /></g><path d="M20 46c0 24 38 24 38 0" fill="none" stroke="#ded4ff" strokeWidth="15" strokeLinecap="round" /></svg></span>
    </div>
    <div className="space-preview-content">
      <div className="space-preview-heading"><span className="space-preview-name">Space</span></div>
      <div className="space-preview-widgets" aria-hidden="true">
        <div className="space-preview-row"><span className="space-preview-widget-icon space-preview-home-icon"><svg viewBox="0 0 24 24"><path d="m2 11 10-8 4 3.2V3h3v5.6l3 2.4-1.8 2.2L12 6.6l-8.2 6.6zM5 12l7-5.5 7 5.5v9h-5v-6h-4v6H5z" /></svg></span><span>Homepage</span></div>
        <div className="space-preview-row"><span className="space-preview-widget-icon">🔑</span><span>Private</span></div>
        <div className="space-preview-row"><span className="space-preview-widget-icon">💼</span><span>Work</span></div>
      </div>
    </div>
  </div>;
}

export function Preferences({ values = {}, onChange, selectedTab, onSelectTab, customThemes = [], themeDirectory = "", customThemeDisabled = false }) {
  const inputValue = (id, fallbackProps) => {
    if (!hasValue(values, id)) return fallbackProps;
    if (onChange) return { ...fallbackProps, value: values[id], onChange: event => onChange(id, event.currentTarget.value, event) };
    return { ...fallbackProps, defaultValue: values[id] };
  };

  return <Tabs label="Preferences" className="advanced-tabs preference-tabs" items={preferenceTabs}
    selected={selectedTab} onSelect={onSelectTab} useChildPanels>
    <SettingsSection className="settings-group accent-group" aria-labelledby="accent-heading">
      <SettingsHeading id="accent-heading" title="Accent Color" description="Choose a color for highlights and active controls." />
      <SettingsGroup>
        {settingSwitch(values, onChange, "include-selections", "Include selections", undefined, {
          id: "include-selections-help",
          text: "Use the accent color for text, table and block selections, selected choices, and focus indicators.",
        })}
        {settingSwitch(values, onChange, "native-accent", "Anytype Accent Color")}
        <div className="color-editor" id="color-editor">
          <ColorPlane />
          <div className="color-controls">
            <label className="hue-field" htmlFor="accent-hue">Hue</label>
            <input id="accent-hue" className="hue-slider" type="range" min="0" max="359" defaultValue="300" aria-label="Color hue" />
            <label className="color-value-label" htmlFor="accent-value">Hex or CSS color</label>
            <div className="color-value-row">
              <span id="accent-swatch" className="accent-swatch" aria-hidden="true" />
              <input id="accent-value" className="color-text-input" type="text" spellCheck="false" autoComplete="off" aria-describedby="accent-error" {...inputValue("accent-value", {})} />
            </div>
          </div>
        </div>
      </SettingsGroup>
      <p className="field-error" id="accent-error" aria-live="polite" />
      <CustomThemes values={values} onChange={onChange} themes={customThemes} directory={themeDirectory} disabled={customThemeDisabled} />
    </SettingsSection>

    <SettingsSection className="settings-group typography-group" aria-labelledby="typography-heading">
      <SettingsHeading id="typography-heading" title="Font" description="Use the same fonts as your Anytype workspace." />
      <SettingsGroup className="font-fields">
        <TextField label="Interface font" inputAttributes={inputValue("font-family", { id: "font-family", type: "text", maxLength: 80, autoComplete: "off", spellCheck: false, placeholder: "Inter" })} />
        <TextField label="Code font" inputAttributes={inputValue("code-font-family", { id: "code-font-family", type: "text", maxLength: 80, autoComplete: "off", spellCheck: false, placeholder: "Consolas" })} />
      </SettingsGroup>
      <p className="field-error" id="font-error" aria-live="polite" />
    </SettingsSection>

    <SettingsSection className="settings-group space-banner-group" aria-labelledby="space-banner-heading">
      <SettingsHeading id="space-banner-heading" title="Space Banner" description="Choose an image to show at the top of your Anytype sidebar." />
      <div className="space-banner-controls">
        <div className="space-banner-actions">
          <Button id="choose-banner-image" className="banner-image-button">Choose image</Button>
          <Button id="reset-banner-image" className="banner-reset-button" hidden>Use default</Button>
          <input id="banner-image-input" className="sr-only" type="file" accept="image/png,image/jpeg,image/webp,.png,.jpg,.jpeg,.webp" aria-label="Choose a space banner image" />
        </div>
        <p id="banner-error" className="field-error" role="alert" aria-live="polite" />
        <SpaceBannerPreview />
        <p id="banner-image-status" className="space-banner-status" role="status" aria-live="polite" />
      </div>
    </SettingsSection>
  </Tabs>;
}

export function CalendarEvents({ values = {}, onChange, selectedTab, onSelectTab, controller }) {
  return <Tabs label="Calendar Events" className="advanced-tabs" items={calendarTabs}
    selected={selectedTab} onSelect={onSelectTab} useChildPanels>
    <SettingsSection aria-labelledby="calendar-general-heading">
      <SettingsHeading id="calendar-general-heading" title="General" />
      <SettingsGroup>
        {settingSwitch(values, onChange, "calendar-colors-enabled", "Calendar Colors", undefined, undefined, {
          defaultChecked: true, disabled: true, "aria-describedby": "calendar-colors-help",
        })}
      </SettingsGroup>
      <p id="calendar-colors-help" className="kanban-collapse-help" role="status" aria-live="polite" hidden />
    </SettingsSection>
    <SettingsSection aria-label="Color by property">
      <CalendarColors controller={controller} />
    </SettingsSection>
  </Tabs>;
}

export function KanbanViews({ values = {}, onChange, selectedTab, onSelectTab, kanbanStyle = "default",
  onKanbanStyleChange, kanbanStyleDisabled, formulaController,
}) {
  const [formula, setFormula] = useState(() => formulaController?.getSnapshot());
  useEffect(() => formulaController?.subscribe(setFormula), [formulaController]);

  return <Tabs label="Kanban Views" className="advanced-tabs" items={kanbanTabs}
    selected={selectedTab} onSelect={onSelectTab} useChildPanels>
    <SettingsSection aria-labelledby="kanban-general-heading">
      <SettingsHeading id="kanban-general-heading" title="General" />
      <SettingsGroup className="settings-list kanban-settings-list">
        <div className="kanban-setting-row kanban-style-row">
          <div className="kanban-setting-copy"><span className="kanban-setting-title">Board layout</span><span id="kanban-style-help" className="kanban-setting-description settings-row-description">Choose the spacing used on inline and full-page boards.</span></div>
          <SelectPicker id="kanban-view-style" rootId="kanban-select" menuId="kanban-style-options"
            label="Board layout" labelId="kanban-style-label" valueId="kanban-style-value" descriptionId="kanban-style-help"
            options={boardLayouts} value={kanbanStyle} disabled={kanbanStyleDisabled} onChange={onKanbanStyleChange} />
        </div>
        <div className="kanban-setting-row formula-display-row">
          <div className="kanban-setting-copy"><span className="kanban-setting-title">Display formulas</span><span id="formula-display-help" className="kanban-setting-description settings-row-description">Choose where category calculation results appear.</span></div>
          <SelectPicker id="formula-display" rootId="formula-display-select" menuId="formula-display-options"
            label="Display formulas" labelId="formula-display-label" valueId="formula-display-value" descriptionId="formula-display-help"
            options={formulaLayouts} value={formula?.display ?? "balanced"} disabled={formula?.locked}
            onChange={value => formulaController?.setDisplay(value)} />
        </div>
        {settingSwitch(values, onChange, "collapsed-categories", "Collapsed Categories", "native-accent-control settings-row kanban-setting-row", undefined, { "aria-describedby": "kanban-collapse-help" })}
      </SettingsGroup>
      <p id="kanban-collapse-help" className="kanban-collapse-help">Show category headers; cards appear on hover or keyboard focus. Touch devices keep cards visible.</p>
    </SettingsSection>
    <SettingsSection className="settings-group kanban-calculations-page" aria-label="Kanban Calculations">
      <SettingsHeading title="Kanban calculations" help="Calculations use the optional Anytype Local API v2 connection. Saved results are snapshots until you save again and rebuild in Anytype." />
      <FormulaPageView controller={formulaController} />
    </SettingsSection>
  </Tabs>;
}

export function Accessibility({ values = {}, onChange }) {
  return <>
    <SettingsSection aria-labelledby="reduced-motion-heading">
      <SettingsHeading id="reduced-motion-heading" title="Reduced Motion" description="Stops BlobUI’s animated text effects to reduce motion." />
      <SettingsGroup>
        {settingSwitch(values, onChange, "reduced-motion", "Reduced Motion", "native-accent-control settings-row accessibility-row")}
      </SettingsGroup>
    </SettingsSection>
    <SettingsSection aria-labelledby="hover-effects-heading">
      <SettingsHeading id="hover-effects-heading" title="Hover Effects" description="Choose which hover previews appear in Anytype." />
      <SettingsGroup>
        {settingSwitch(values, onChange, "tab-hover-effect", "Tab hover effect", "native-accent-control settings-row accessibility-row")}
        {settingSwitch(values, onChange, "link-hover-effect", "Links hover effect", "native-accent-control settings-row accessibility-row")}
        {settingSwitch(values, onChange, "object-mention-hover-effect", "Object mention hover effect", "native-accent-control settings-row accessibility-row")}
      </SettingsGroup>
      <p className="accessibility-note">BlobUI’s <code>blob://</code> links never show hover previews.</p>
    </SettingsSection>
  </>;
}
