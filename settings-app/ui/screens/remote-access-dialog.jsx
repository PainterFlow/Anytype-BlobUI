import React from "react";
import { Button, SettingsGroup, SettingsRow } from "../shared/settings-controls.jsx";
import "./remote-access-dialog.css";

export function RemoteAccessDialog({
  dialogRef,
  closeRef,
  remoteToken,
  remoteTokenVisible,
  remoteCopied,
  tunnelUrl,
  tunnelChanging,
  tunnelCopied,
  status,
  onClose,
  onToggleRemoteToken,
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
      <SettingsGroup>
        <SettingsRow
          className="settings-row anyblob-remote-dialog-row"
          label="Remote access key"
          description="Only shown after enabling remote access. Disable and enable again to rotate it."
        >
          <Button className="banner-image-button" disabled={!remoteToken} onClick={onCopyRemoteToken}>
            {remoteCopied ? "Copied" : "Copy key"}
          </Button>
        </SettingsRow>
        {remoteToken && <div className="settings-row anyblob-remote-dialog-row">
          <input
            id="anyblob-remote-access-key"
            aria-label="Remote Anyblob key"
            type={remoteTokenVisible ? "text" : "password"}
            value={remoteToken}
            readOnly
            autoComplete="off"
          />
          <Button className="banner-image-button" aria-controls="anyblob-remote-access-key" onClick={onToggleRemoteToken}>
            {remoteTokenVisible ? "Hide" : "Show"}
          </Button>
        </div>}
      </SettingsGroup>
      <SettingsGroup>
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
          <input aria-label="Cloudflare Tunnel URL" value={tunnelUrl} readOnly />
          <Button className="banner-image-button" onClick={onCopyTunnelUrl}>
            {tunnelCopied ? "Copied" : "Copy URL"}
          </Button>
        </div>}
      </SettingsGroup>
      <p className="anyblob-remote-dialog-status" role="status">{status}</p>
      <div className="anyblob-remote-dialog-instructions">
        <h3>Setup instructions</h3>
        <p>Install cloudflared on this PC from <a href="https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/" target="_blank" rel="noreferrer">Cloudflare's download instructions</a>, then start the Quick Tunnel above. Quick Tunnels need no Cloudflare account and stop working when BlobUI closes or you stop the tunnel.</p>
        <p>On the remote Codex machine, build the MCP server and set <code>ANYBLOB_REMOTE_ADDRESS</code> to the tunnel URL and <code>ANYBLOB_REMOTE_TOKEN</code> to this key. Protect your Codex config because the token grants Anyblob access under saved global permissions.</p>
        <p>Quick Tunnel URLs are temporary. For a stable URL, create a named Cloudflare Tunnel and configure it to route to this PC's local Anyblob HTTP bridge at <code>http://127.0.0.1:31421</code>. Keep its origin limited to loopback.</p>
      </div>
    </div>
  </section>;
}
