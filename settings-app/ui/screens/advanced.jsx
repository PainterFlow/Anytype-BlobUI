import React from "react";
import {
  Button,
  SettingsGroup,
  SettingsHeading,
  SettingsRow,
  SettingsSection,
  Switch,
  VisibilityTextField,
} from "../shared/settings-controls.jsx";
import { SelectPicker } from "../shared/select-picker.jsx";
import { Tabs } from "../shared/tabs.jsx";

const advancedTabs = [
  { title: "API key", tabId: "advanced-api-tab", panelId: "advanced-api-panel" },
  { title: "App settings", tabId: "advanced-app-tab", panelId: "advanced-app-panel" },
  { title: "Experiments", tabId: "advanced-experiments-tab", panelId: "advanced-experiments-panel" },
];

export function Advanced({ selectedTab, onSelectTab, startupMode = "off", startupModeDisabled = true, onStartupModeChange, backgroundExperiment = "disabled", backgroundExperimentDisabled = true, onBackgroundExperimentChange }) {
  return <Tabs id="advanced-tabs" label="Advanced settings" items={advancedTabs}
    selected={selectedTab} onSelect={onSelectTab} useChildPanels>
    <SettingsSection>
      <SettingsHeading id="advanced-api-heading" title="API key" description="Configure the local API connection used by BlobUI." />
      <SettingsGroup className="settings-list advanced-fields">
        <VisibilityTextField label="Anytype API key" inputAttributes={{
          id: "advanced-api-key",
          type: "password",
          autoComplete: "new-password",
          spellCheck: false,
          maxLength: 4096,
          "aria-describedby": "advanced-api-key-hint advanced-api-error",
        }} buttonAttributes={{
          id: "advanced-api-key-visibility",
          className: "visibility-toggle",
          "aria-controls": "advanced-api-key",
          "aria-pressed": "false",
          "aria-label": "Show Anytype API key",
          title: "Show Anytype API key",
        }} />
        <VisibilityTextField label="API key IP" inputAttributes={{
          id: "advanced-api-ip",
          type: "text",
          inputMode: "decimal",
          autoComplete: "off",
          spellCheck: false,
          maxLength: 45,
          placeholder: "127.0.0.1",
          "aria-describedby": "advanced-api-error",
        }} buttonAttributes={{
          id: "advanced-api-ip-visibility",
          className: "visibility-toggle",
          "aria-controls": "advanced-api-ip",
          "aria-pressed": "true",
          "aria-label": "Hide API key IP",
          title: "Hide API key IP",
        }} />
        <VisibilityTextField label="API key port" inputAttributes={{
          id: "advanced-api-port",
          type: "text",
          inputMode: "numeric",
          autoComplete: "off",
          spellCheck: false,
          maxLength: 5,
          placeholder: "31009",
          "aria-describedby": "advanced-api-error",
        }} buttonAttributes={{
          id: "advanced-api-port-visibility",
          className: "visibility-toggle",
          "aria-controls": "advanced-api-port",
          "aria-pressed": "true",
          "aria-label": "Hide API key port",
          title: "Hide API key port",
        }} />
      </SettingsGroup>
      <p id="advanced-api-key-hint" className="accessibility-note">Enter a new key to replace the saved key. Clear the masked value to remove it.</p>
      <p id="advanced-api-error" className="field-error" role="alert" aria-live="polite" />
    </SettingsSection>

    <SettingsSection>
      <SettingsSection aria-labelledby="app-general-heading">
        <SettingsHeading id="app-general-heading" title="General" />
        <section className="app-general-settings">
          {backgroundExperiment === "autostart-background" ? <SettingsRow className="native-accent-control settings-row accessibility-row" label="Launch at startup">
            <SelectPicker id="startup-mode" label="Launch at startup" value={startupMode} disabled={startupModeDisabled}
              options={[{ value: "off", label: "Don't autostart" }, { value: "foreground", label: "Autostart" }, { value: "background", label: "Autostart in background" }]}
              onChange={onStartupModeChange} />
          </SettingsRow> : <Switch id="start-on-login" label="Launch at startup" className="native-accent-control settings-row accessibility-row"
            checked={startupMode !== "off"} disabled={startupModeDisabled} onChange={checked => onStartupModeChange?.(checked ? "foreground" : "off")} />}
          <Switch id="autoplay-videos" label="Autoplay videos" className="native-accent-control settings-row accessibility-row" />
        </section>
      </SettingsSection>

      <SettingsSection aria-labelledby="anytype-installation-heading">
        <SettingsHeading id="anytype-installation-heading" title="Anytype Folder" description="Choose the Anytype data folder that contains custom.css." />
        <div className="app-install-directory-row">
          <input id="install-directory" className="color-text-input" type="text" readOnly role="button" aria-haspopup="dialog" aria-label="Anytype Folder" autoComplete="off" spellCheck={false} maxLength={1024} placeholder="Select Anytype Folder" aria-describedby="updater-settings-error" />
        </div>
        <p id="updater-settings-error" className="field-error" role="alert" aria-live="polite" />
      </SettingsSection>

      <SettingsSection aria-labelledby="updates-heading">
        <SettingsHeading id="updates-heading" title="Updates" description="Choose how BlobUI checks for and installs updates." />
        <SettingsGroup>
          <Switch id="automatic-updates-enabled" label="Install updates automatically" className="native-accent-control settings-row accessibility-row" />
          <Switch id="automatic-checks-enabled" label="Check for updates automatically" className="native-accent-control settings-row accessibility-row" />
        </SettingsGroup>
        <div className="update-check-row">
          <div>
            <p id="update-version" className="update-version" />
            <p id="update-status" className="update-status" role="status" aria-live="polite">Loading update settings…</p>
          </div>
          <Button id="check-updates-button" className="banner-image-button update-check-button" disabled>Check for updates</Button>
        </div>
      </SettingsSection>

      <SettingsSection aria-labelledby="backup-heading">
        <SettingsHeading id="backup-heading" title="Backup and import" />
        <SettingsGroup className="settings-list app-backup-box">
          <SettingsRow className="settings-row app-backup-row" label="Export settings" description="Create a dated copy of the complete BlobUI folder, including settings and calculations." descriptionAttributes={{ className: "app-backup-description" }}>
            <Button id="create-blob-backup-button" className="banner-image-button blob-backup-button" disabled>Export</Button>
          </SettingsRow>
          <SettingsRow className="settings-row app-backup-row" label="Import settings" description="Replace the current BlobUI folder with an exported backup." descriptionAttributes={{ className: "app-backup-description" }}>
            <Button id="import-blob-backup-button" className="banner-image-button blob-backup-button" disabled>Import</Button>
          </SettingsRow>
        </SettingsGroup>
        <p id="blob-backup-status" className="update-status blob-backup-status" role="status" aria-live="polite" />
      </SettingsSection>

      <SettingsSection aria-labelledby="app-uninstall-heading">
        <SettingsHeading id="app-uninstall-heading" title="Uninstall" />
        <Button id="uninstall-button" className="banner-image-button danger-button">Uninstall BlobUI</Button>
      </SettingsSection>
    </SettingsSection>
    <SettingsSection aria-labelledby="experiments-heading">
      <div className="section-heading">
        <h2 id="experiments-heading">Experiments</h2>
        <p>Experimental features may change while they are in development.</p>
        <p id="experiments-warning" className="experiments-warning">
          <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M8.5 3a1.7 1.7 0 0 1 3 0l6 11a1.7 1.7 0 0 1-1.5 2.5H4a1.7 1.7 0 0 1-1.5-2.5Z" /><path d="M10 7v4" /><circle cx="10" cy="13.5" r=".7" fill="currentColor" stroke="none" /></svg>
          <span>Experiments may break things. Use at your own risk.</span>
        </p>
      </div>
      <SettingsGroup>
        <SettingsRow className="native-accent-control settings-row accessibility-row">
          <span>Run in background</span>
          <SelectPicker id="background-experiment" label="Run in background" value={backgroundExperiment}
            disabled={backgroundExperimentDisabled} descriptionId="experiments-warning" onChange={onBackgroundExperimentChange}
            options={[
              { value: "disabled", label: "Disabled", description: "Use standard Settings controls." },
              { value: "normal", label: "Variant 1", description: "Normal. Adds the title-bar background button." },
              { value: "autostart-background", label: "Variant 2", description: "Autostart Background. Also enables background startup." },
            ]} />
        </SettingsRow>
      </SettingsGroup>
    </SettingsSection>
  </Tabs>;
}
