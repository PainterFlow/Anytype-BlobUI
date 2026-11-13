import React, { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { Switch } from "../shared/settings-controls.jsx";

const UI=()=>window.BlobUI;

export function CodexIntegration() {
  const page=UI().CodexPage,api=useSyncExternalStore(page.subscribe,page.getSnapshot,page.getSnapshot);
  const [enabled,setEnabled]=useState(false),[installation,setInstallation]=useState(null),[status,setStatus]=useState(""),[changing,setChanging]=useState(false),[installing,setInstalling]=useState(false),[completion,setCompletion]=useState(null);
  const [remote,setRemote]=useState(false),[remoteToken,setRemoteToken]=useState(""),[remoteTokenVisible,setRemoteTokenVisible]=useState(false),[remoteChanging,setRemoteChanging]=useState(false),[remoteCopied,setRemoteCopied]=useState(false),[tunnelUrl,setTunnelUrl]=useState(""),[tunnelChanging,setTunnelChanging]=useState(false),[tunnelCopied,setTunnelCopied]=useState(false);
  const integrationEpoch=useRef(0),installationEpoch=useRef(0),backdrop=useRef(null),dialog=useRef(null),done=useRef(null),modal=useRef(null);
  const refresh=async()=>{if(!api)return;const epoch=++integrationEpoch.current;try{const value=await api.invoke("anyblob_status");if(epoch===integrationEpoch.current){setEnabled(value.enabled);setRemote(value.remoteAccess===true);}}catch(error){if(epoch===integrationEpoch.current)setStatus(String(error.message||error));}};
  const refreshInstallation=async()=>{if(!api||installing)return;const epoch=++installationEpoch.current;try{const value=await api.invoke("anyblob_installation_status");if(epoch===installationEpoch.current)setInstallation(value);}catch(error){if(epoch===installationEpoch.current){setInstallation(null);setStatus(String(error.message||error));}}};
  useEffect(()=>{if(!api)return;void refresh();void refreshInstallation();return()=>{integrationEpoch.current++;installationEpoch.current++;};},[api]);
  useEffect(()=>UI().AppReload?.register(()=>Promise.allSettled([refresh(),refreshInstallation()])),[api,installing]);
  useEffect(()=>{const listener=()=>{void refresh();void refreshInstallation();};window.addEventListener("blobui-codex-integration-refresh",listener);return()=>window.removeEventListener("blobui-codex-integration-refresh",listener);},[api]);
  useEffect(()=>{if(!backdrop.current||!dialog.current)return;modal.current=UI().createModalController({backdrop:backdrop.current,dialog:dialog.current,initialFocus:()=>done.current,inertTargets:[document.getElementById("settings-react-root")],onClose:()=>setCompletion(null)});return()=>{modal.current?.destroy();modal.current=null;};},[api]);
  useEffect(()=>{if(completion)modal.current?.open();else if(modal.current?.isOpen())modal.current.close();},[completion]);
  async function configure(value){if(!api||changing)return;setChanging(true);integrationEpoch.current++;setStatus("");try{const result=await api.invoke("configure_anyblob",{enabled:value});setEnabled(result.enabled);setRemote(false);setRemoteToken("");setRemoteTokenVisible(false);setTunnelUrl("");}catch(error){setStatus(String(error.message||error));await refresh();}finally{setChanging(false);}}
  async function configureRemote(value){if(!api||remoteChanging)return;setRemoteChanging(true);setStatus("");setRemoteCopied(false);try{const result=await api.invoke("configure_anyblob_remote",{enabled:value});setRemote(result.enabled);setRemoteToken(result.token||"");setRemoteTokenVisible(false);if(!value)setTunnelUrl("");}catch(error){setStatus(String(error.message||error));await refresh();}finally{setRemoteChanging(false);}}
  async function copyRemoteToken(){try{await navigator.clipboard.writeText(remoteToken);setRemoteCopied(true);}catch(error){setStatus("Could not copy the remote key. Select and copy it manually.");}}
  async function startTunnel(){if(!api||tunnelChanging)return;setTunnelChanging(true);setStatus("");try{const result=await api.invoke("start_cloudflare_tunnel");setTunnelUrl(result.url);setTunnelCopied(false);}catch(error){setStatus(String(error.message||error));}finally{setTunnelChanging(false);}}
  async function stopTunnel(){if(!api||tunnelChanging)return;setTunnelChanging(true);try{await api.invoke("stop_cloudflare_tunnel");setTunnelUrl("");}catch(error){setStatus(String(error.message||error));}finally{setTunnelChanging(false);}}
  async function copyTunnelUrl(){try{await navigator.clipboard.writeText(tunnelUrl);setTunnelCopied(true);}catch(error){setStatus("Could not copy the tunnel URL. Select and copy it manually.");}}
  async function installMarketplace(){if(!api||installing)return;const updating=!!installation?.updateAvailable;setInstalling(true);installationEpoch.current++;setStatus("");try{await api.invoke("install_anyblob_marketplace");const next=await api.invoke("anyblob_installation_status");setInstallation(next);setCompletion({updating});}catch(error){setStatus(String(error.message||error));}finally{setInstalling(false);}}
  const refreshRequired=installation?.installed && installation?.codexRefreshRequired && !installation?.updateAvailable;
  if(!api)return null;
  return <>
    <div className="section-heading"><h2>General</h2></div>
    <div className="settings-list">
      <Switch id="enable-anyblob" label="Enable Anyblob" className="native-accent-control settings-row" checked={enabled} disabled={changing} onChange={configure} />
      <div className="settings-row kanban-setting-row"><div className="kanban-setting-copy"><span className="kanban-setting-title">Install Marketplace</span><span className="anyblob-version settings-row-description" hidden={!installation?.installedVersion&&!installation?.updateAvailable}>{installation?.installedVersion&&<>v{installation.installedVersion}</>}{installation?.updateAvailable&&installation?.bundledVersion&&installation.bundledVersion!==installation.installedVersion&&<>{installation.installedVersion?" → ":""}<span className="anyblob-version-available">v{installation.bundledVersion}</span></>}</span></div><button type="button" className="banner-image-button update-check-button" disabled={installing} title={installation?.bundledVersion?`Anyblob ${installation.bundledVersion}`:""} onClick={()=>refreshRequired?setCompletion({refresh:true}):void installMarketplace()}>{installing?(installation?.updateAvailable?"Updating…":"Installing…"):installation?.updateAvailable?"Update":refreshRequired?"Refresh in Codex":installation?.installed?"Installed":"Install"}</button></div>
    </div>
    {enabled&&<>
      <div className="section-heading"><h2>Remote Codex</h2></div>
      <div className="settings-list">
        <Switch id="enable-anyblob-remote" label="Allow Cloudflare Tunnel remote access" className="native-accent-control settings-row" checked={remote} disabled={remoteChanging} onChange={configureRemote} />
      </div>
      <p className="input-hint">The tunnel exposes only the authenticated Anyblob bridge. It does not expose Anytype's Local API or API key. Remote chats use saved global space permissions. Project-specific permissions are unavailable.</p>
      {remote&&<div className="settings-list anyblob-remote-setup">
        <div className="settings-row kanban-setting-row"><div className="kanban-setting-copy"><span className="kanban-setting-title">Remote access key</span><span className="settings-row-description">Only shown after enabling remote access. Disable and enable again to rotate it.</span></div><button type="button" className="banner-image-button" disabled={!remoteToken} onClick={copyRemoteToken}>{remoteCopied?"Copied":"Copy key"}</button></div>
        {remoteToken&&<label className="settings-row"><span className="kanban-setting-title">Key</span><input aria-label="Remote Anyblob key" type={remoteTokenVisible?"text":"password"} value={remoteToken} readOnly autoComplete="off"/><button type="button" className="banner-image-button" onClick={()=>setRemoteTokenVisible(value=>!value)}>{remoteTokenVisible?"Hide":"Show"}</button></label>}
        <div className="settings-row kanban-setting-row"><div className="kanban-setting-copy"><span className="kanban-setting-title">Cloudflare Quick Tunnel</span><span className="settings-row-description">Install cloudflared first. The temporary URL changes each time the tunnel starts.</span></div><button type="button" className="banner-image-button" disabled={tunnelChanging} onClick={tunnelUrl?stopTunnel:startTunnel}>{tunnelChanging?(tunnelUrl?"Stopping…":"Starting…"):tunnelUrl?"Stop tunnel":"Start tunnel"}</button></div>
        {tunnelUrl&&<label className="settings-row"><span className="kanban-setting-title">Tunnel URL</span><input aria-label="Cloudflare Tunnel URL" value={tunnelUrl} readOnly/><button type="button" className="banner-image-button" onClick={copyTunnelUrl}>{tunnelCopied?"Copied":"Copy URL"}</button></label>}
        <div className="input-hint"><p>Install cloudflared on this PC from <a href="https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/" target="_blank" rel="noreferrer">Cloudflare's download instructions</a>, then start the Quick Tunnel above. Quick Tunnels need no Cloudflare account and stop working when BlobUI closes or you stop the tunnel.</p><p>On the remote Codex machine, build the MCP server and set <code>ANYBLOB_REMOTE_ADDRESS</code> to the tunnel URL and <code>ANYBLOB_REMOTE_TOKEN</code> to this key. Protect your Codex config because the token grants Anyblob access under saved global permissions.</p><p>Quick Tunnel URLs are temporary. For a stable URL, create a named Cloudflare Tunnel and configure it to route to this PC's local Anyblob HTTP bridge at <code>http://127.0.0.1:31421</code>. Keep its origin limited to loopback.</p></div>
      </div>}
    </>}
    <p className="input-hint" role="status">{status}</p>
    {createPortal(<div ref={backdrop} className="confirm-backdrop" hidden>
      <section ref={dialog} className="confirm-dialog" role="dialog" aria-modal="true" aria-label={completion?.refresh?"Refresh Anyblob in Codex":completion?.updating?"Anyblob marketplace updated":"Anyblob marketplace installed"} tabIndex={-1}>
        <h2>{completion?.refresh?"Refresh Anyblob in Codex":completion?.updating?"Anyblob marketplace updated":"Anyblob marketplace installed"}</h2>
        <p>{completion?.refresh?"The marketplace package is up to date. Open Codex's Plugins Directory and refresh Anyblob. If it still uses the previous copy, uninstall and reinstall Anyblob there, then reopen Codex.":completion?.updating?"The bundled Anyblob release is registered. Open Codex’s Plugins Directory and refresh Anyblob. If it still uses the previous release, uninstall and reinstall Anyblob there, then reopen Codex. BlobUI does not replace Codex’s running plugin copy.":"Open the Plugins Directory, select your personal marketplace, then choose Anyblob to install and enable it. You may need to reopen Codex. Space access still requires saved BlobUI permissions."}</p>
        <div className="confirm-actions"><button ref={done} type="button" className="confirm-cancel" onClick={()=>modal.current?.close()}>Done</button></div>
      </section>
    </div>, document.body)}
  </>;
}
