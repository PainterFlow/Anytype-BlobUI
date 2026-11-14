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
    inputAttributes={{ id: "anyblob-remote-access-key", ref: input, type: "password", defaultValue: value, readOnly: true, autoComplete: "off", spellCheck: false }}
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
          label="Cloudflare Quick Tunnel"
          description="Install cloudflared first. The temporary URL changes each time the tunnel starts."
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
            inputAttributes={{ "aria-label": "Cloudflare Tunnel URL" }}
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
        <p>Install cloudflared on this PC from <a href="https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/" target="_blank" rel="noreferrer">Cloudflare's download instructions</a>, then start the Quick Tunnel above. Quick Tunnels need no Cloudflare account and stop working when BlobUI closes or you stop the tunnel.</p>
        <p>On the remote Codex machine, build the MCP server and set <code>ANYBLOB_REMOTE_ADDRESS</code> to the tunnel URL and <code>ANYBLOB_REMOTE_TOKEN</code> to this key. Protect your Codex config because the token grants access under saved BlobUI permissions.</p>
        <p>Launch or reconnect the remote Anyblob MCP server after configuring its URL and key. It syncs its host and project catalog at startup and refreshes the catalog when the remote Codex project list changes. Sync sends project IDs and names, not project folder paths. BlobUI checks request membership against the synced catalog separately from tool arguments.</p>
        <p>In BlobUI Settings &gt; Anyblob &gt; Projects, cloud rows appear under the remote PC name. The globe tooltip shows the computer name. Select a project row to save source permissions for that host and project. These project scopes override global space permissions for chats with verified membership in configured projects. Projectless chats and projects without saved sources use global permissions. Missing or invalid identity is denied when project scopes are saved.</p>
        <p>Quick Tunnel URLs are temporary. For a stable URL, create a named Cloudflare Tunnel and configure it to route to this PC's local Anyblob HTTP bridge at <code>http://127.0.0.1:31421</code>. Keep its origin limited to loopback.</p>
      </div>
    </div>
  </section>;
}
