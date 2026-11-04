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
import { Tabs } from "../shared/tabs.jsx";

const advancedTabs = [
  { title: "API key", tabId: "advanced-api-tab", panelId: "advanced-api-panel" },
  { title: "App settings", tabId: "advanced-app-tab", panelId: "advanced-app-panel" },
];

export function Advanced({ selectedTab, onSelectTab }) {
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
          <Switch id="start-on-login" label="Launch at startup" className="native-accent-control settings-row accessibility-row" />
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
  </Tabs>;
}
