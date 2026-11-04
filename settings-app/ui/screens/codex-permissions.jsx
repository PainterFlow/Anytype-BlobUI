import React, { useState, useSyncExternalStore } from "react";
import { ExpandableSearch } from "../shared/search.jsx";
import { Switch } from "../shared/settings-controls.jsx";
import { PermissionPicker } from "./codex-projects.jsx";

const UI=()=>window.BlobUI;
const access=[{value:"none",label:"No Access"},{value:"read",label:"Read Only"},{value:"readWrite",label:"Read-Write"},{value:"createDelete",label:"Create-Delete"}];
const approval=[{value:"alwaysAsk",label:"Always Ask"},{value:"approveForMe",label:"Approve for me"},{value:"fullAccess",label:"Never Ask"}];

function SpaceRow({space,permission,disabled,onChange}) {
  const description=`${space.name}\n${space.id}`;
  const value=permission||{access:"none",filesEnabled:false,approval:"approveForMe"};
  return <section className="formula-view codex-space">
    <div className="codex-space-top"><span className="formula-view-name" title={description}>{space.name}</span></div>
    <div className="codex-space-bottom">
      <Switch id={`codex-files-${space.id}`} label="Upload/delete files" className="native-accent-control settings-row" inputAttributes={{"aria-label":`Upload/delete files for ${description}`}} checked={value.filesEnabled} disabled={disabled} onChange={checked=>onChange({...value,filesEnabled:checked})} />
      <PermissionPicker id={description} label="Access" options={access} value={value.access} disabled={disabled} onChange= {next=>onChange({...value,access:next})} />
      <PermissionPicker id={description} label="Approval" options={approval} value={value.approval} disabled={disabled} onChange={next=>onChange({...value,approval:next})} />
    </div>
  </section>;
}

export function CodexPermissions() {
  const controller=UI().CodexPage;
  const state=useSyncExternalStore(controller.subscribePermissions,controller.getPermissionsSnapshot,controller.getPermissionsSnapshot);
  const api=controller.getSnapshot();
  const [query,setQuery]=useState("");
  const preferences=api?controller.normalize(api.read()):{spaces:{}};
  const spaces=state.spaces.filter(space=>`${space.name} ${space.id}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase()));
  const openApiKey=()=>{api.navigate("advanced");document.getElementById("advanced-api-tab")?.click();};
  return <>
    <div className="formula-toolbar">
      <ExpandableSearch id="codex-search" label="Search spaces" placeholder="Search spaces" value={query} onChange={setQuery} disabled={state.locked} />
      <div id="codex-filter-control" hidden />
      <button id="codex-reload" type="button" className="neutral-icon-button" aria-label="Reload list" title="Reload list" disabled={state.locked||state.busy} aria-busy={String(state.busy)} onClick={()=>controller.reloadSpaces()}><svg viewBox="0 0 20 20" aria-hidden="true"><path d="M16 5.5v4h-4M15.8 9.2a6 6 0 1 0-.7 4" fill="none" /></svg></button>
    </div>
    <p className="input-hint" role="status">{state.message}</p>
    {state.message.includes("API key")&&<button type="button" className="formula-button" onClick={openApiKey}>Open Advanced Settings &gt; API key</button>}
    <div id="codex-space-list">{spaces.map(space=><SpaceRow key={space.id} space={space} permission={preferences.spaces[space.id]} disabled={state.locked} onChange={value=>controller.changePermission(space.id,value)} />)}{state.spaces.length>0&&!spaces.length&&<p className="input-hint">No matching results.</p>}</div>
  </>;
}
