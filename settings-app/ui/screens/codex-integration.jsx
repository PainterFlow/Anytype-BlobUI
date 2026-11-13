import React, { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { DialogSwitch, Switch } from "../shared/settings-controls.jsx";
import { RemoteAccessDialog } from "./remote-access-dialog.jsx";

const UI=()=>window.BlobUI;

export function CodexIntegration() {
  const page=UI().CodexPage,api=useSyncExternalStore(page.subscribe,page.getSnapshot,page.getSnapshot);
  const [enabled,setEnabled]=useState(false),[installation,setInstallation]=useState(null),[status,setStatus]=useState(""),[changing,setChanging]=useState(false),[installing,setInstalling]=useState(false),[completion,setCompletion]=useState(null);
  const [remote,setRemote]=useState(false),[remoteToken,setRemoteToken]=useState(""),[remoteChanging,setRemoteChanging]=useState(false),[remoteCopied,setRemoteCopied]=useState(false),[tunnelUrl,setTunnelUrl]=useState(""),[tunnelChanging,setTunnelChanging]=useState(false),[tunnelCopied,setTunnelCopied]=useState(false),[remoteDialogRequested,setRemoteDialogRequested]=useState(false);
  const integrationEpoch=useRef(0),installationEpoch=useRef(0),backdrop=useRef(null),dialog=useRef(null),done=useRef(null),modal=useRef(null),remoteBackdrop=useRef(null),remoteDialog=useRef(null),remoteClose=useRef(null),remoteModal=useRef(null);
  const refresh=async()=>{if(!api)return;const epoch=++integrationEpoch.current;try{const value=await api.invoke("anyblob_status");if(epoch===integrationEpoch.current){setEnabled(value.enabled);setRemote(value.enabled&&value.remoteAccess===true);}}catch(error){if(epoch===integrationEpoch.current)setStatus(String(error.message||error));}};
  const refreshInstallation=async()=>{if(!api||installing)return;const epoch=++installationEpoch.current;try{const value=await api.invoke("anyblob_installation_status");if(epoch===installationEpoch.current)setInstallation(value);}catch(error){if(epoch===installationEpoch.current){setInstallation(null);setStatus(String(error.message||error));}}};
  useEffect(()=>{if(!api)return;void refresh();void refreshInstallation();return()=>{integrationEpoch.current++;installationEpoch.current++;};},[api]);
  useEffect(()=>UI().AppReload?.register(()=>Promise.allSettled([refresh(),refreshInstallation()])),[api,installing]);
  useEffect(()=>{const listener=()=>{void refresh();void refreshInstallation();};window.addEventListener("blobui-codex-integration-refresh",listener);return()=>window.removeEventListener("blobui-codex-integration-refresh",listener);},[api]);
  useEffect(()=>{if(!backdrop.current||!dialog.current)return;modal.current=UI().createModalController({backdrop:backdrop.current,dialog:dialog.current,initialFocus:()=>done.current,inertTargets:[document.getElementById("settings-react-root")],onClose:()=>setCompletion(null)});return()=>{modal.current?.destroy();modal.current=null;};},[api]);
  useEffect(()=>{if(!remoteBackdrop.current||!remoteDialog.current)return;remoteModal.current=UI().createModalController({backdrop:remoteBackdrop.current,dialog:remoteDialog.current,initialFocus:()=>remoteClose.current,fallbackFocus:()=>document.getElementById("enable-anyblob"),inertTargets:[document.getElementById("settings-react-root")],onClose:()=>setRemoteDialogRequested(false)});return()=>{remoteModal.current?.destroy();remoteModal.current=null;};},[api]);
  useEffect(()=>{if(completion)modal.current?.open();else if(modal.current?.isOpen())modal.current.close();},[completion]);
  useEffect(()=>{if(remoteDialogRequested&&enabled&&remote&&!changing&&!remoteChanging)remoteModal.current?.open();else if(remoteModal.current?.isOpen())remoteModal.current.close();},[remoteDialogRequested,enabled,remote,changing,remoteChanging]);
  async function configure(value){if(!api||changing)return;if(!value){setRemoteDialogRequested(false);remoteModal.current?.close();}setChanging(true);integrationEpoch.current++;setStatus("");try{const result=await api.invoke("configure_anyblob",{enabled:value});setEnabled(result.enabled);setRemote(false);setRemoteToken("");setTunnelUrl("");setRemoteDialogRequested(false);}catch(error){setStatus(String(error.message||error));await refresh();}finally{setChanging(false);}}
  async function configureRemote(value){if(!api||remoteChanging||(value&&!enabled))return;if(!value){setRemoteDialogRequested(false);remoteModal.current?.close();}setRemoteChanging(true);setStatus("");setRemoteCopied(false);try{const result=await api.invoke("configure_anyblob_remote",{enabled:value});setRemote(result.enabled===true);setRemoteToken(result.token||"");if(!value){setTunnelUrl("");setRemoteDialogRequested(false);}else if(result.enabled===true){setRemoteDialogRequested(true);}}catch(error){setStatus(String(error.message||error));await refresh();}finally{setRemoteChanging(false);}}
  function openRemoteDialog(){if(enabled&&remote&&!changing&&!remoteChanging)setRemoteDialogRequested(true);}
  async function copyRemoteToken(){try{await navigator.clipboard.writeText(remoteToken);setRemoteCopied(true);}catch(error){setStatus("Could not copy the remote key. Select and copy it manually.");}}
  async function startTunnel(){if(!api||!enabled||!remote||tunnelChanging)return;setTunnelChanging(true);setStatus("");try{const result=await api.invoke("start_cloudflare_tunnel");setTunnelUrl(result.url);setTunnelCopied(false);}catch(error){setStatus(String(error.message||error));}finally{setTunnelChanging(false);}}
  async function stopTunnel(){if(!api||!enabled||!remote||tunnelChanging)return;setTunnelChanging(true);try{await api.invoke("stop_cloudflare_tunnel");setTunnelUrl("");}catch(error){setStatus(String(error.message||error));}finally{setTunnelChanging(false);}}
  async function copyTunnelUrl(){try{await navigator.clipboard.writeText(tunnelUrl);setTunnelCopied(true);}catch(error){setStatus("Could not copy the tunnel URL. Select and copy it manually.");}}
  async function installMarketplace(){if(!api||installing)return;const updating=!!installation?.updateAvailable;setInstalling(true);installationEpoch.current++;setStatus("");try{await api.invoke("install_anyblob_marketplace");const next=await api.invoke("anyblob_installation_status");setInstallation(next);setCompletion({updating});}catch(error){setStatus(String(error.message||error));}finally{setInstalling(false);}}
  const refreshRequired=installation?.installed && installation?.codexRefreshRequired && !installation?.updateAvailable;
  if(!api)return null;
  return <>
    <div className="section-heading"><h2>General</h2></div>
    <div className="settings-list anyblob-integration-settings">
      <Switch id="enable-anyblob" label="Enable Anyblob" className="native-accent-control settings-row" checked={enabled} disabled={changing} onChange={configure} />
      <div className="settings-row kanban-setting-row"><div className="kanban-setting-copy"><span className="kanban-setting-title">Install Marketplace</span><span className="anyblob-version settings-row-description" hidden={!installation?.installedVersion&&!installation?.updateAvailable}>{installation?.installedVersion&&<>v{installation.installedVersion}</>}{installation?.updateAvailable&&installation?.bundledVersion&&installation.bundledVersion!==installation.installedVersion&&<>{installation.installedVersion?" → ":""}<span className="anyblob-version-available">v{installation.bundledVersion}</span></>}</span></div><button type="button" className="banner-image-button update-check-button" disabled={installing} title={installation?.bundledVersion?`Anyblob ${installation.bundledVersion}`:""} onClick={()=>refreshRequired?setCompletion({refresh:true}):void installMarketplace()}>{installing?(installation?.updateAvailable?"Updating…":"Installing…"):installation?.updateAvailable?"Update":refreshRequired?"Refresh in Codex":installation?.installed?"Installed":"Install"}</button></div>
    </div>
    <div className="settings-list anyblob-integration-settings">
      <DialogSwitch id="enable-anyblob-remote" dialogId="anyblob-remote-access-dialog" label="Allow Cloudflare Tunnel remote access"
        help="The tunnel exposes only the authenticated Anyblob bridge. It does not expose Anytype's Local API or API key. Remote chats use saved global space permissions. Project-specific permissions are unavailable."
        checked={remote} disabled={!enabled||changing||remoteChanging} labelDisabled={!enabled||changing||remoteChanging||!remote}
        expanded={remoteDialogRequested&&enabled&&remote} onOpen={openRemoteDialog} onChange={configureRemote} />
    </div>
    <p className="input-hint" role="status">{status}</p>
    {createPortal(<div ref={remoteBackdrop} className="confirm-backdrop" hidden>
      <RemoteAccessDialog dialogRef={remoteDialog} closeRef={remoteClose} remoteToken={remoteToken}
        remoteCopied={remoteCopied} tunnelUrl={tunnelUrl} tunnelChanging={tunnelChanging} tunnelCopied={tunnelCopied} status={status}
        onClose={()=>remoteModal.current?.close()} onCopyRemoteToken={copyRemoteToken}
        onStartTunnel={startTunnel} onStopTunnel={stopTunnel} onCopyTunnelUrl={copyTunnelUrl} />
    </div>, document.body)}
    {createPortal(<div ref={backdrop} className="confirm-backdrop" hidden>
      <section ref={dialog} className="confirm-dialog" role="dialog" aria-modal="true" aria-label={completion?.refresh?"Refresh Anyblob in Codex":completion?.updating?"Anyblob marketplace updated":"Anyblob marketplace installed"} tabIndex={-1}>
        <h2>{completion?.refresh?"Refresh Anyblob in Codex":completion?.updating?"Anyblob marketplace updated":"Anyblob marketplace installed"}</h2>
        <p>{completion?.refresh?"The marketplace package is up to date. Open Codex's Plugins Directory and refresh Anyblob. If it still uses the previous copy, uninstall and reinstall Anyblob there, then reopen Codex.":completion?.updating?"The bundled Anyblob release is registered. Open Codex’s Plugins Directory and refresh Anyblob. If it still uses the previous release, uninstall and reinstall Anyblob there, then reopen Codex. BlobUI does not replace Codex’s running plugin copy.":"Open the Plugins Directory, select your personal marketplace, then choose Anyblob to install and enable it. You may need to reopen Codex. Space access still requires saved BlobUI permissions."}</p>
        <div className="confirm-actions"><button ref={done} type="button" className="confirm-cancel" onClick={()=>modal.current?.close()}>Done</button></div>
      </section>
    </div>, document.body)}
  </>;
}
