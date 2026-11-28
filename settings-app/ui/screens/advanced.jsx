import React from "react";
import {
  Button,
  FolderPathInput,
  SettingsGroup,
  SettingsHeading,
  SettingsRow,
  SettingsSection,
  Switch,
  VisibilityTextField,
} from "../shared/settings-controls.jsx";
import { SelectPicker } from "../shared/select-picker.jsx";
import { Tabs } from "../shared/tabs.jsx";
import { HelpTooltip } from "../shared/help-tooltip.jsx";

const advancedTabs = [
  { title: "API Key", tabId: "advanced-api-tab", panelId: "advanced-api-panel" },
  { title: "App Settings", tabId: "advanced-app-tab", panelId: "advanced-app-panel" },
  { title: "Experiments", tabId: "advanced-experiments-tab", panelId: "advanced-experiments-panel" },
];

export function Advanced({
  selectedTab, onSelectTab, startupMode = "off", startupModeDisabled = true, onStartupModeChange,
  releaseChannel = "release", releaseChannelDisabled = true, onReleaseChannelChange,
  backgroundExperiment = "disabled", backgroundExperimentDisabled = true, onBackgroundExperimentChange,
  fullscreenExperiment = "disabled", fullscreenExperimentDisabled = true, onFullscreenExperimentChange,
  alwaysStartFullscreen = false, alwaysStartFullscreenDisabled = true, onAlwaysStartFullscreenChange,
  appSettings = {}, appSettingsActions = {},
}) {
  const {
    updateVersionText = "", updateStatusText = "Loading update settings…", updateStatusError = false,
    checkUpdatesLabel = "Check for updates", checkUpdatesDisabled = true,
    updaterControlsDisabled = false, automaticUpdatesChecked = false, automaticChecksChecked = false,
    backupActionsDisabled = true, blobBackupStatusText = "", blobBackupStatusError = false,
    installDirectoryValue = "", installDirectoryDisabled = false, installDirectoryInvalid = false, installDirectoryBusy = false,
    clearCacheDisabled = true, uninstallDisabled = false, updaterSettingsError = "",
  } = appSettings;
  const {
    onCheckUpdates, onAutomaticUpdatesChange, onAutomaticChecksChange,
    onCreateBlobBackup, onImportBlobBackup, onSelectAnytypeFolder, onInstallDirectoryInput,
    onClearCache, onUninstall,
  } = appSettingsActions;
  return <Tabs id="advanced-tabs" label="Advanced settings" items={advancedTabs}
    selected={selectedTab} onSelect={onSelectTab} useChildPanels>
    <SettingsSection>
      <SettingsHeading id="advanced-api-heading" title="API Key" description="Configure the local API connection used by BlobUI." />
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
      <SettingsSection aria-labelledby="updates-heading">
        <div className="section-heading">
          <div className="section-heading-title"><h2 id="updates-heading">Updates</h2></div>
        </div>
        <SettingsGroup className="settings-list update-current-box">
          <div className="update-check-row">
            <div>
              <p id="update-version" className="update-version">{updateVersionText}</p>
              <p id="update-status" className={updateStatusError ? "update-status error" : "update-status"} role="status" aria-live="polite">{updateStatusText}</p>
            </div>
            <Button id="check-updates-button" className="banner-image-button update-check-button" disabled={checkUpdatesDisabled} onClick={onCheckUpdates}>{checkUpdatesLabel}</Button>
          </div>
        </SettingsGroup>
        <div className="section-heading update-settings-heading"><p>Choose how BlobUI checks for and installs updates.</p></div>
        <SettingsGroup>
          <Switch id="automatic-updates-enabled" label="Install updates automatically" className="native-accent-control settings-row accessibility-row"
            checked={automaticUpdatesChecked} disabled={updaterControlsDisabled} onChange={onAutomaticUpdatesChange} />
          <Switch id="automatic-checks-enabled" label="Check for updates automatically" className="native-accent-control settings-row accessibility-row"
            checked={automaticChecksChecked} disabled={updaterControlsDisabled} onChange={onAutomaticChecksChange} />
          <SettingsRow className="native-accent-control settings-row accessibility-row" label="Release Channel">
            <SelectPicker id="release-channel" label="Release Channel" value={releaseChannel} disabled={releaseChannelDisabled}
              options={[{ value: "release", label: "Release" }, { value: "dev", label: "Dev Builds" }]}
              onChange={onReleaseChannelChange} fixed />
          </SettingsRow>
        </SettingsGroup>
      </SettingsSection>

      <SettingsSection aria-labelledby="backup-heading">
        <SettingsHeading id="backup-heading" title="Backup and import" />
        <SettingsGroup className="settings-list app-backup-box">
          <SettingsRow className="settings-row app-backup-row" label="Export settings" description="Create a dated copy of the complete BlobUI folder, including settings and calculations." descriptionAttributes={{ className: "app-backup-description" }}>
            <Button id="create-blob-backup-button" className="banner-image-button blob-backup-button" disabled={backupActionsDisabled} onClick={onCreateBlobBackup}>Export</Button>
          </SettingsRow>
          <SettingsRow className="settings-row app-backup-row" label="Import settings">
            <Button id="import-blob-backup-button" className="banner-image-button blob-backup-button" disabled={backupActionsDisabled} onClick={onImportBlobBackup}>Import</Button>
          </SettingsRow>
        </SettingsGroup>
        <p id="blob-backup-status" className={blobBackupStatusError ? "update-status blob-backup-status error" : "update-status blob-backup-status"} role="status" aria-live="polite">{blobBackupStatusText}</p>
      </SettingsSection>

      <SettingsSection aria-labelledby="installation-heading">
        <SettingsHeading id="installation-heading" title="Instillation" />
        <SettingsGroup className="settings-list installation-settings-box">
          <SettingsRow className="native-accent-control settings-row installation-folder-row" label="Anytype Folder" description="Choose the Anytype data folder that contains custom.css.">
            <FolderPathInput id="install-directory" aria-label="Anytype Folder" placeholder="Select Anytype Folder" aria-describedby="updater-settings-error"
              value={installDirectoryValue} disabled={installDirectoryDisabled} className={installDirectoryInvalid ? "invalid" : ""}
              aria-busy={installDirectoryBusy ? "true" : undefined} onClick={() => onSelectAnytypeFolder?.()}
              onKeyDown={event => {
                if (event.key !== "Enter" && event.key !== " ") return;
                event.preventDefault();
                onSelectAnytypeFolder?.();
              }}
              onChange={event => onInstallDirectoryInput?.(event.currentTarget.value)} />
          </SettingsRow>
          {backgroundExperiment === "autostart-background" ? <SettingsRow className="native-accent-control settings-row accessibility-row" label="Launch at startup">
            <SelectPicker id="startup-mode" label="Launch at startup" value={startupMode} disabled={startupModeDisabled}
              options={[{ value: "off", label: "Don't autostart" }, { value: "foreground", label: "Autostart" }, { value: "background", label: "Autostart in background" }]}
              onChange={onStartupModeChange} />
          </SettingsRow> : <Switch id="start-on-login" label="Launch at startup" className="native-accent-control settings-row accessibility-row"
            checked={startupMode !== "off"} disabled={startupModeDisabled} onChange={checked => onStartupModeChange?.(checked ? "foreground" : "off")} />}
          {fullscreenExperiment === "enable-full-screen" && <SettingsRow className="native-accent-control settings-row accessibility-row">
            <span className="experiment-label-copy"><span>Always start in fullscreen</span><HelpTooltip variant="warning" label="Always start in fullscreen warning" text="Experimental option, when enabled it always starts the app in windowed mode. Expect bugs, but it should cause no actual issues." /></span>
            <Switch id="always-start-fullscreen" label="Always start in fullscreen" className="native-accent-control settings-row compact-switch-row" inputAttributes={{"aria-label":"Always start in fullscreen"}} checked={alwaysStartFullscreen} disabled={alwaysStartFullscreenDisabled} onChange={checked => onAlwaysStartFullscreenChange?.(checked)} />
          </SettingsRow>}
          <SettingsRow className="native-accent-control settings-row accessibility-row" label="Clear Cache">
            <Button id="clear-cache-button" className="banner-image-button clear-cache-button" disabled={clearCacheDisabled} onClick={onClearCache}>Clear Cache</Button>
          </SettingsRow>
        </SettingsGroup>
        <SettingsGroup className="settings-list app-uninstall-box">
          <SettingsRow className="settings-row app-backup-row" label="Uninstall BlobUI">
            <Button id="uninstall-button" className="banner-image-button danger-button uninstall-button" disabled={uninstallDisabled} onClick={onUninstall}>Uninstall</Button>
          </SettingsRow>
        </SettingsGroup>
        <p id="updater-settings-error" className="field-error" role="alert" aria-live="polite">{updaterSettingsError}</p>
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
          <span className="experiment-label-copy"><span>Run in background</span><HelpTooltip label="Run in background help" text="Allows the app to run in the background to save performance and remove rarely used windows. Should have 0 risk using, variant 2 integrates it with autostarting." /></span>
          <SelectPicker id="background-experiment" label="Run in background" value={backgroundExperiment}
            disabled={backgroundExperimentDisabled} descriptionId="experiments-warning" onChange={onBackgroundExperimentChange}
            options={[
              { value: "disabled", label: "Disabled" },
              { value: "normal", label: "Background Button" },
              { value: "autostart-background", label: "Autostart in Background" },
            ]} />
        </SettingsRow>
        <SettingsRow className="native-accent-control settings-row accessibility-row">
          <span className="experiment-label-copy"><span>Re-introduce Fullscreen</span><HelpTooltip label="Re-introduce Fullscreen help" text="Fixes a few bugs with UI rescale in variant 1, variant 2 goes further and allows using re-introduces full windowed mode. Low risk using, expect UI bugs though." /></span>
          <SelectPicker id="fullscreen-experiment" label="Re-introduce Fullscreen" value={fullscreenExperiment}
            disabled={fullscreenExperimentDisabled} descriptionId="experiments-warning" onChange={onFullscreenExperimentChange}
            options={[
              { value: "disabled", label: "Disabled" },
              { value: "fix-resizing", label: "Fix Resizing" },
              { value: "enable-full-screen", label: "Enable Full Screen" },
            ]} />
        </SettingsRow>
      </SettingsGroup>
    </SettingsSection>
  </Tabs>;
}
