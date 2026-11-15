import React, { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { clearStaleSaveStatus } from "../anyblob-status.js";
import { Button, FolderPathInput, SettingsGroup, SettingsRow, Switch } from "../shared/settings-controls.jsx";
import "./codex-integration.css";

const UI=()=>window.BlobUI;

export function CodexIntegration() {
  const page=UI().CodexPage,api=useSyncExternalStore(page.subscribe,page.getSnapshot,page.getSnapshot);
  const [enabled,setEnabled]=useState(false),[installation,setInstallation]=useState(null),[status,setStatus]=useState(""),[changing,setChanging]=useState(false),[installing,setInstalling]=useState(false),[completion,setCompletion]=useState(null);
  const [folder, setFolder] = useState("");
  const [folderChanging, setFolderChanging] = useState(false);
  const [tunnel, setTunnel] = useState(null);
  const [tunnelEnabled, setTunnelEnabled] = useState(false);
  const [tunnelId, setTunnelId] = useState("");
  const [runtimeApiKey, setRuntimeApiKey] = useState("");
  const [tunnelChanging, setTunnelChanging] = useState(false);
  const [tunnelError, setTunnelError] = useState("");
  const integrationEpoch = useRef(0), installationEpoch = useRef(0), tunnelEpoch = useRef(0);
  const tunnelSettingsLoaded = useRef(false);
  const backdrop = useRef(null), dialog = useRef(null), done = useRef(null), modal = useRef(null);

  const refresh = async () => {
    if (!api) return;
    const epoch = ++integrationEpoch.current;
    try {
      const value = await api.invoke("anyblob_status");
      if (epoch !== integrationEpoch.current) return;
      setEnabled(value.enabled);
      setFolder(value.localProjectsFolder || "");
      setStatus(clearStaleSaveStatus);
    } catch (error) {
      if (epoch === integrationEpoch.current) setStatus(String(error.message || error));
    }
  };
  const refreshInstallation=async()=>{if(!api||installing)return;const epoch=++installationEpoch.current;try{const value=await api.invoke("anyblob_installation_status");if(epoch===installationEpoch.current)setInstallation(value);}catch(error){if(epoch===installationEpoch.current){setInstallation(null);setStatus(String(error.message||error));}}};
  const refreshTunnel = async () => {
    if (!api) return;
    const epoch = ++tunnelEpoch.current;
    try {
      const value = await api.invoke("secure_mcp_tunnel_status");
      if (epoch !== tunnelEpoch.current) return;
      setTunnel(value);
      if (!tunnelSettingsLoaded.current) {
        tunnelSettingsLoaded.current = true;
        setTunnelEnabled(value.enabled);
        setTunnelId(value.tunnelId || "");
      }
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
      tunnelSettingsLoaded.current = false;
    };
  }, [api]);
  useEffect(() => UI().AppReload?.register(() => Promise.allSettled([refresh(), refreshInstallation(), refreshTunnel()])), [api, installing]);
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

  async function configure(value) {
    if (!api || changing || folderChanging || tunnelChanging) return;
    setChanging(true);
    integrationEpoch.current++;
    setStatus("");
    try {
      const result = await api.invoke("configure_anyblob", { enabled: value });
      setEnabled(result.enabled);
      if (!result.enabled) setRuntimeApiKey("");
      await refreshTunnel();
    } catch (error) {
      await refresh();
      setStatus(String(error.message || error));
    } finally {
      setChanging(false);
    }
  }

  async function browseFolder() {
    if (!enabled || changing || folderChanging) return;
    setFolderChanging(true);
    setStatus("");
    try {
      const selected = await api.invoke("select_anyblob_projects_folder", { currentDirectory: folder || null });
      if (selected == null) return;
      integrationEpoch.current++;
      await api.invoke("set_anyblob_projects_folder", { folder: selected });
      setFolder(selected);
      void UI().ProjectPermissions?.reloadProjects();
    } catch (error) {
      setStatus(String(error.message || error));
    } finally {
      setFolderChanging(false);
    }
  }

  async function configureTunnel(value) {
    if (!enabled || tunnelChanging || changing) return;
    setTunnelChanging(true);
    setTunnelError("");
    tunnelEpoch.current++;
    try {
      await api.invoke("configure_secure_mcp_tunnel", {
        enabled: value,
        tunnelId: tunnelId.trim(),
        runtimeApiKey: value ? runtimeApiKey.trim() || null : null,
      });
      setTunnelEnabled(value);
      setRuntimeApiKey("");
    } catch (error) {
      setTunnelError(String(error.message || error));
    } finally {
      await refreshTunnel();
      setTunnelChanging(false);
    }
  }

  function changeTunnelEnabled(value) {
    setTunnelError("");
    if (!value && (tunnel?.running || tunnel?.enabled)) {
      void configureTunnel(false);
      return;
    }
    setTunnelEnabled(value);
    if (!value) setRuntimeApiKey("");
  }

  async function installMarketplace(){if(!api||installing)return;const updating=!!installation?.updateAvailable;setInstalling(true);installationEpoch.current++;setStatus("");try{await api.invoke("install_anyblob_marketplace");const next=await api.invoke("anyblob_installation_status");setInstallation(next);setCompletion({updating});}catch(error){setStatus(String(error.message||error));}finally{setInstalling(false);}}
  const refreshRequired=installation?.installed && installation?.codexRefreshRequired && !installation?.updateAvailable;
  if (!api) return null;
  const tunnelLocked = !enabled || changing || tunnelChanging;
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
      <Switch id="enable-anyblob" label="Enable Anyblob" className="native-accent-control settings-row" checked={enabled} disabled={changing || folderChanging || tunnelChanging} onChange={configure} />
      <div className="settings-row kanban-setting-row"><div className="kanban-setting-copy"><span className="kanban-setting-title">Install Marketplace</span><span className="anyblob-version settings-row-description" hidden={!installation?.installedVersion&&!installation?.updateAvailable}>{installation?.installedVersion&&<>v{installation.installedVersion}</>}{installation?.updateAvailable&&installation?.bundledVersion&&installation.bundledVersion!==installation.installedVersion&&<>{installation.installedVersion?" → ":""}<span className="anyblob-version-available">v{installation.bundledVersion}</span></>}</span></div><button type="button" className="banner-image-button update-check-button" disabled={installing} title={installation?.bundledVersion?`Anyblob ${installation.bundledVersion}`:""} onClick={()=>refreshRequired?setCompletion({refresh:true}):void installMarketplace()}>{installing?(installation?.updateAvailable?"Updating…":"Installing…"):installation?.updateAvailable?"Update":refreshRequired?"Refresh in Codex":installation?.installed?"Installed":"Install"}</button></div>
    </div>
    <SettingsGroup className="settings-list anyblob-integration-settings">
      <SettingsRow className="settings-row anyblob-folder-row" label="Local Projects Folder">
        <FolderPathInput id="anyblob-local-projects-folder" aria-label="Local Projects Folder" value={folder}
          placeholder="No folder selected" disabled={!enabled || changing || folderChanging} onClick={browseFolder}
          onKeyDown={event => {
            if (event.key === "Enter" || event.key === " ") {
              event.preventDefault();
              void browseFolder();
            }
          }} />
        <Button className="banner-image-button" aria-haspopup="dialog" aria-label="Browse for Local Projects Folder"
          disabled={!enabled || changing || folderChanging} onClick={browseFolder}>Browse</Button>
      </SettingsRow>
    </SettingsGroup>
    <SettingsGroup className="settings-list anyblob-integration-settings">
      <Switch id="enable-secure-mcp-tunnel" label="Secure MCP Tunnel" checked={enabled && tunnelEnabled}
        disabled={tunnelLocked || !tunnel} onChange={changeTunnelEnabled} />
      <SettingsRow label={<label htmlFor="anyblob-tunnel-id">Tunnel ID</label>}>
        <input id="anyblob-tunnel-id" className="color-text-input anyblob-tunnel-input" value={tunnelId}
          autoComplete="off" spellCheck={false} disabled={tunnelFieldsDisabled}
          onChange={event => setTunnelId(event.currentTarget.value)} />
      </SettingsRow>
      <SettingsRow label={<label htmlFor="anyblob-tunnel-key">Runtime API key</label>}>
        <input id="anyblob-tunnel-key" className="color-text-input anyblob-tunnel-input" type="password"
          autoComplete="new-password" spellCheck={false} value={runtimeApiKey}
          placeholder={tunnel?.keyConfigured ? "Key saved" : ""} disabled={tunnelFieldsDisabled}
          onChange={event => setRuntimeApiKey(event.currentTarget.value)} />
      </SettingsRow>
      <SettingsRow description={tunnelError || tunnelStatus} label={tunnel?.running ? "Tunnel running" : "Tunnel stopped"}
        descriptionAttributes={{ role: "status" }}>
        <Button className="banner-image-button" disabled={tunnelLocked || (!tunnel?.running && !canStart)}
          onClick={() => void configureTunnel(!tunnel?.running)}>
          {tunnelChanging ? "Please wait…" : tunnel?.running ? "Stop tunnel" : "Start tunnel"}
        </Button>
      </SettingsRow>
    </SettingsGroup>
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
