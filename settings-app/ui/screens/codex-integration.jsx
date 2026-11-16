import React, { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { clearStaleSaveStatus } from "../anyblob-status.js";
import { Button, DialogSwitch, FolderPathInput, SecretTextField, SettingsGroup, SettingsRow, Switch } from "../shared/settings-controls.jsx";
import { SettingsDialog } from "../shared/settings-dialog.jsx";
import "./codex-integration.css";

const UI=()=>window.BlobUI;

export function CodexIntegration() {
  const page=UI().CodexPage,api=useSyncExternalStore(page.subscribe,page.getSnapshot,page.getSnapshot);
  const draftController = UI().AnyblobDraft;
  const draft = useSyncExternalStore(draftController.subscribe, draftController.getSnapshot, draftController.getSnapshot);
  const { enabled, folder, tunnelEnabled, tunnelId, runtimeApiKey } = draft;
  const setTunnelId = value => draftController.change({ tunnelId: value });
  const setRuntimeApiKey = value => draftController.change({ runtimeApiKey: value });
  const [installation,setInstallation]=useState(null),[status,setStatus]=useState(""),[installing,setInstalling]=useState(false),[completion,setCompletion]=useState(null);
  const [folderChanging, setFolderChanging] = useState(false);
  const [tunnel, setTunnel] = useState(null);
  const [tunnelChanging, setTunnelChanging] = useState(false);
  const [tunnelError, setTunnelError] = useState("");
  const [tunnelOpen, setTunnelOpen] = useState(false);
  const integrationEpoch = useRef(0), installationEpoch = useRef(0), tunnelEpoch = useRef(0);
  const backdrop = useRef(null), dialog = useRef(null), done = useRef(null), modal = useRef(null);

  const refresh = async () => {
    if (!api) return;
    const epoch = ++integrationEpoch.current;
    try {
      await draftController.load();
      if (epoch !== integrationEpoch.current) return;
      setStatus(clearStaleSaveStatus);
    } catch (error) {
      if (epoch === integrationEpoch.current) setStatus(String(error.message || error));
    }
  };
  const refreshInstallation=async()=>{if(!api||installing)return;const epoch=++installationEpoch.current;try{const value=await api.invoke("anyblob_installation_status");if(epoch===installationEpoch.current)setInstallation(value);}catch(error){if(epoch===installationEpoch.current){setInstallation(null);setStatus(String(error.message||error));}}};
  const refreshTunnel = async (restart = null) => {
    if (!api) return;
    const epoch = ++tunnelEpoch.current;
    try {
      const value = restart === null
        ? await api.invoke("secure_mcp_tunnel_status")
        : await api.invoke("refresh_secure_mcp_tunnel", { restart });
      if (epoch !== tunnelEpoch.current) return;
      setTunnel(value);
      if (restart !== null) setTunnelError("");
    } catch (error) {
      if (epoch === tunnelEpoch.current) setTunnelError(String(error.message || error));
    }
  };
  useEffect(() => {
    if (!api) return;
    void refresh();
    void refreshInstallation();
    void refreshTunnel();
    return () => {
      integrationEpoch.current++;
      installationEpoch.current++;
      tunnelEpoch.current++;
    };
  }, [api]);
  useEffect(() => UI().AppReload?.register(() => Promise.allSettled([refresh(), refreshInstallation(), tunnelChanging ? undefined : refreshTunnel(false)])), [api, installing, tunnelChanging]);
  useEffect(() => {
    const listener = () => { void refresh(); void refreshInstallation(); void refreshTunnel(); };
    window.addEventListener("blobui-codex-integration-refresh", listener);
    return () => window.removeEventListener("blobui-codex-integration-refresh", listener);
  }, [api]);
  useEffect(() => {
    if (!api || !tunnel?.running || tunnelChanging) return;
    const timer = window.setInterval(() => void refreshTunnel(), 1500);
    return () => window.clearInterval(timer);
  }, [api, tunnel?.running, tunnelChanging]);
  useEffect(()=>{if(!backdrop.current||!dialog.current)return;modal.current=UI().createModalController({backdrop:backdrop.current,dialog:dialog.current,initialFocus:()=>done.current,inertTargets:[document.getElementById("settings-react-root")],onClose:()=>setCompletion(null)});return()=>{modal.current?.destroy();modal.current=null;};},[api]);
  useEffect(()=>{if(completion)modal.current?.open();else if(modal.current?.isOpen())modal.current.close();},[completion]);

  function configure(value) {
    draftController.change({ enabled: value });
  }

  async function browseFolder() {
    if (!enabled || folderChanging) return;
    setFolderChanging(true);
    setStatus("");
    try {
      const selected = await api.invoke("select_anyblob_projects_folder", { currentDirectory: folder || null });
      if (selected == null) return;
      integrationEpoch.current++;
      draftController.change({ folder: selected });
    } catch (error) {
      setStatus(String(error.message || error));
    } finally {
      setFolderChanging(false);
    }
  }

  async function configureTunnel(action) {
    if (!enabled || tunnelChanging) return;
    setTunnelChanging(true);
    setTunnelError("");
    tunnelEpoch.current++;
    try {
      if (action === "restart") await refreshTunnel(true);
      else if (action === "start") await draftController.save({ startTunnel: true });
      else await draftController.stop();
    } catch (error) {
      setTunnelError(String(error.message || error));
    } finally {
      await refreshTunnel();
      setTunnelChanging(false);
    }
  }

  function changeTunnelEnabled(value) {
    setTunnelError("");
    draftController.change({ tunnelEnabled: value });
  }

  async function installMarketplace(){if(!api||installing)return;const updating=!!installation?.updateAvailable;setInstalling(true);installationEpoch.current++;setStatus("");try{await api.invoke("install_anyblob_marketplace");const next=await api.invoke("anyblob_installation_status");setInstallation(next);setCompletion({updating});}catch(error){setStatus(String(error.message||error));}finally{setInstalling(false);}}
  const refreshRequired=installation?.installed && installation?.codexRefreshRequired && !installation?.updateAvailable;
  if (!api) return null;
  const tunnelLocked = !enabled || draft.locked || tunnelChanging;
  const tunnelFieldsDisabled = tunnelLocked || !tunnelEnabled || !!tunnel?.running;
  const canStart = tunnelEnabled && tunnel?.runtimeAvailable && tunnelId.trim() &&
    (runtimeApiKey.trim() || (tunnel.keyConfigured && tunnelId.trim() === tunnel.tunnelId));
  let tunnelStatus = "";
  if (enabled && tunnel) {
    if (tunnel.ready) tunnelStatus = "Tunnel is ready.";
    else if (tunnel.running) tunnelStatus = "Connecting to OpenAI…";
    else if (!tunnel.runtimeAvailable) tunnelStatus = "Install OpenAI tunnel-client to use the tunnel.";
    else if (!tunnelId.trim() || !(runtimeApiKey.trim() || tunnel.keyConfigured)) tunnelStatus = "Enter a tunnel ID and runtime API key to start.";
    else tunnelStatus = "Tunnel is stopped.";
  }
  return <>
    <div className="section-heading"><h2>General</h2></div>
    <div className="settings-list anyblob-integration-settings">
      <Switch id="enable-anyblob" label="Enable Anyblob" className="native-accent-control settings-row" checked={enabled} disabled={draft.locked || folderChanging || tunnelChanging} onChange={configure} />
      <div className="settings-row kanban-setting-row"><div className="kanban-setting-copy"><span className="kanban-setting-title">Install Marketplace</span><span className="anyblob-version settings-row-description" hidden={!installation?.installedVersion&&!installation?.updateAvailable}>{installation?.installedVersion&&<>v{installation.installedVersion}</>}{installation?.updateAvailable&&installation?.bundledVersion&&installation.bundledVersion!==installation.installedVersion&&<>{installation.installedVersion?" → ":""}<span className="anyblob-version-available">v{installation.bundledVersion}</span></>}</span></div><button type="button" className="banner-image-button update-check-button" disabled={installing} title={installation?.bundledVersion?`Anyblob ${installation.bundledVersion}`:""} onClick={()=>refreshRequired?setCompletion({refresh:true}):void installMarketplace()}>{installing?(installation?.updateAvailable?"Updating…":"Installing…"):installation?.updateAvailable?"Update":refreshRequired?"Refresh in Codex":installation?.installed?"Installed":"Install"}</button></div>
    </div>
    <SettingsGroup className="settings-list anyblob-integration-settings">
      <SettingsRow className="settings-row anyblob-folder-row" label="Local Projects Folder">
        <FolderPathInput id="anyblob-local-projects-folder" aria-label="Local Projects Folder" value={folder}
          placeholder="No folder selected" disabled={!enabled || draft.locked || folderChanging} onClick={browseFolder}
          onKeyDown={event => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              void browseFolder();
            }
          }} />
      </SettingsRow>
    </SettingsGroup>
    <SettingsGroup className="settings-list anyblob-integration-settings">
      <DialogSwitch id="enable-secure-mcp-tunnel" label="Secure MCP Tunnel" checked={enabled && tunnelEnabled}
        dialogId="anyblob-tunnel-dialog" expanded={tunnelOpen} disabled={tunnelLocked || !tunnel}
        onOpen={() => setTunnelOpen(true)} onChange={value => { changeTunnelEnabled(value); if (value) setTunnelOpen(true); }} />
    </SettingsGroup>
    <SettingsDialog id="anyblob-tunnel-dialog" title="Secure MCP Tunnel" open={tunnelOpen} onClose={() => setTunnelOpen(false)} className="anyblob-tunnel-dialog">
      <div className="anyblob-tunnel-fields">
        <SecretTextField label="Tunnel ID" resetKey={tunnelOpen} className="input-field anyblob-tunnel-field"
          inputAttributes={{ id: "anyblob-tunnel-id", className: "color-text-input", value: tunnelId,
            autoComplete: "off", spellCheck: false, disabled: tunnelFieldsDisabled,
            onChange: event => setTunnelId(event.currentTarget.value) }} />
        <SecretTextField label="Runtime API key" resetKey={tunnelOpen} className="input-field anyblob-tunnel-field"
          inputAttributes={{ id: "anyblob-tunnel-key", className: "color-text-input", value: runtimeApiKey,
            autoComplete: "new-password", spellCheck: false, disabled: tunnelFieldsDisabled,
            placeholder: tunnel?.keyConfigured ? "Key saved" : "",
            onChange: event => setRuntimeApiKey(event.currentTarget.value) }} />
      </div>
      <SettingsRow className="settings-row anyblob-tunnel-actions" description={tunnelError || tunnelStatus} label={tunnel?.running ? "Tunnel running" : "Tunnel stopped"}
        descriptionAttributes={{ role: "status" }}>
        <div className="anyblob-tunnel-buttons">
          <Button className="banner-image-button" disabled={tunnelLocked || (!tunnel?.running && !canStart)}
            onClick={() => void configureTunnel(tunnel?.running ? "stop" : "start")}>
            {tunnelChanging ? "Please wait…" : tunnel?.running ? "Stop tunnel" : "Start tunnel"}
          </Button>
          {tunnel?.running && <Button className="banner-image-button" disabled={tunnelLocked}
            onClick={() => void configureTunnel("restart")}>Restart tunnel</Button>}
        </div>
      </SettingsRow>
    </SettingsDialog>
    <p className="input-hint" role="status" id="anyblob-integration-status">{status}</p>
    {createPortal(<div ref={backdrop} className="confirm-backdrop" hidden>
      <section ref={dialog} className="confirm-dialog" role="dialog" aria-modal="true" aria-label={completion?.refresh?"Refresh Anyblob in Codex":completion?.updating?"Anyblob marketplace updated":"Anyblob marketplace installed"} tabIndex={-1}>
        <h2>{completion?.refresh?"Refresh Anyblob in Codex":completion?.updating?"Anyblob marketplace updated":"Anyblob marketplace installed"}</h2>
        <p>{completion?.refresh?"The marketplace package is up to date. Open Codex's Plugins Directory and refresh Anyblob. If it still uses the previous copy, uninstall and reinstall Anyblob there, then reopen Codex.":completion?.updating?"The bundled Anyblob release is registered. Open Codex’s Plugins Directory and refresh Anyblob. If it still uses the previous release, uninstall and reinstall Anyblob there, then reopen Codex. BlobUI does not replace Codex’s running plugin copy.":"Open the Plugins Directory, select your personal marketplace, then choose Anyblob to install and enable it. You may need to reopen Codex. Space access still requires saved BlobUI permissions."}</p>
        <div className="confirm-actions"><button ref={done} type="button" className="confirm-cancel" onClick={()=>modal.current?.close()}>Done</button></div>
      </section>
    </div>, document.body)}
  </>;
}
