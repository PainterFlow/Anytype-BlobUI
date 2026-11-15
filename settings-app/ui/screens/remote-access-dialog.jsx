import React, { useEffect, useRef } from "react";
import { CopyTextField } from "../shared/copy-text-field.jsx";
import { Button, SettingsRow, VisibilityTextField } from "../shared/settings-controls.jsx";
import "./remote-access-dialog.css";

function RemoteAccessKeyField({ value }) {
  const input = useRef(null);
  const button = useRef(null);

  useEffect(() => {
    if (!input.current || !button.current || !window.VisibilityField) return;
    const controller = window.VisibilityField.bind({
      input: input.current,
      button: button.current,
      field: input.current.closest(".visibility-field"),
      showLabel: "Show remote access key",
      hideLabel: "Hide remote access key",
      defaultVisible: false,
    });
    return () => controller.destroy();
  }, []);

  return <VisibilityTextField label="Key" className="input-field advanced-row anyblob-remote-dialog-key-field"
    readOnly
    inputAttributes={{ id: "anyblob-remote-access-key", ref: input, type: "password", defaultValue: value, autoComplete: "off", spellCheck: false }}
    buttonAttributes={{ ref: button, className: "visibility-toggle", "aria-controls": "anyblob-remote-access-key", "aria-pressed": "false", "aria-label": "Show remote access key", title: "Show remote access key" }} />;
}

export function RemoteAccessDialog({
  dialogRef,
  closeRef,
  remoteToken,
  remoteCopied,
  tunnelUrl,
  tunnelChanging,
  tunnelCopied,
  status,
  onClose,
  onCopyRemoteToken,
  onStartTunnel,
  onStopTunnel,
  onCopyTunnelUrl,
}) {
  const tunnelAction = tunnelUrl ? "Stop tunnel" : "Start tunnel";
  const tunnelBusyLabel = tunnelUrl ? "Stopping…" : "Starting…";

  return <section
    id="anyblob-remote-access-dialog"
    className="confirm-dialog anyblob-remote-dialog"
    role="dialog"
    aria-modal="true"
    aria-labelledby="anyblob-remote-access-title"
    tabIndex={-1}
    ref={dialogRef}
  >
    <header className="anyblob-remote-dialog-header">
      <h2 id="anyblob-remote-access-title">Remote access</h2>
      <button ref={closeRef} type="button" className="neutral-icon-button" aria-label="Close remote access" onClick={onClose}>×</button>
    </header>
    <div className="anyblob-remote-dialog-content">
      <div className="anyblob-remote-dialog-group">
        <SettingsRow
          className="settings-row anyblob-remote-dialog-row"
          label="Remote access key"
          description="Only shown after enabling remote access. Disable and enable again to rotate it."
        >
          <Button className="banner-image-button" disabled={!remoteToken} onClick={onCopyRemoteToken}>
            {remoteCopied ? "Copied" : "Copy key"}
          </Button>
        </SettingsRow>
        {remoteToken && <RemoteAccessKeyField value={remoteToken} />}
      </div>
      <div className="anyblob-remote-dialog-group">
        <SettingsRow
          className="settings-row anyblob-remote-dialog-row"
          label="Tailscale Funnel"
          description="Use Tailscale Funnel to get a stable ts.net address without buying a domain."
        >
          <Button
            className="banner-image-button"
            disabled={tunnelChanging}
            onClick={tunnelUrl ? onStopTunnel : onStartTunnel}
          >
            {tunnelChanging ? tunnelBusyLabel : tunnelAction}
          </Button>
        </SettingsRow>
        {tunnelUrl && <div className="settings-row anyblob-remote-dialog-row">
          <CopyTextField
            value={tunnelUrl}
            inputAttributes={{ "aria-label": "Tailscale Funnel URL" }}
            copied={tunnelCopied}
            copyLabel="Copy tunnel URL"
            copiedLabel="Tunnel URL copied"
            onCopy={onCopyTunnelUrl}
          />
        </div>}
      </div>
      <p className="anyblob-remote-dialog-status" role="status">{status}</p>
      <div className="anyblob-remote-dialog-instructions">
        <h3>Setup instructions</h3>
        <p>Install <a href="https://tailscale.com/download/windows" target="_blank" rel="noreferrer">Tailscale for Windows</a>, sign in to your tailnet, and enable Funnel in its policy. BlobUI requires Tailscale 1.52 or later. Start Funnel above. The <code>.ts.net</code> address stays stable while this device keeps its Tailscale identity and name. BlobUI stops the public route when it closes or when you stop Funnel.</p>
        <p>On the remote Codex machine, build the MCP connector and set <code>ANYBLOB_REMOTE_ADDRESS</code> to this base URL and <code>ANYBLOB_REMOTE_TOKEN</code> to this key. Protect its Codex config because the token grants access under saved BlobUI permissions. Funnel is public, so anyone can reach the bridge, but requests still need this token.</p>
        <p>The remote MCP server only forwards requests. Each MCP request carries its Codex chat ID to local BlobUI, which checks it against the local <code>.codex-global-state.json</code> before applying project permissions. The Anytype Local API and its API key stay on this PC.</p>
        <p>In BlobUI Settings &gt; Anyblob &gt; Projects, remote rows include their Codex host ID. Select a project row to save source permissions for that host and project. These project scopes override global space permissions for chats assigned to configured projects. Projectless chats and projects without saved sources use global permissions. Missing or invalid identity is denied when project scopes are saved.</p>
        <p>Tailscale Funnel uses a public HTTPS endpoint with a stable <code>*.ts.net</code> address. Tailscale limits Funnel to ports <code>443</code>, <code>8443</code>, and <code>10000</code>; BlobUI uses port <code>443</code> and routes only <code>/anyblob</code> to its loopback bridge at <code>http://127.0.0.1:31421</code>. Funnel is currently in beta and Tailscale applies a bandwidth limit.</p>
      </div>
    </div>
  </section>;
}
