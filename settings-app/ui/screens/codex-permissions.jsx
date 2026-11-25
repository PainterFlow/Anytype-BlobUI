import React, { useState, useSyncExternalStore } from "react";
import { PermissionPicker } from "../shared/permission-picker.jsx";
import { Switch } from "../shared/settings-controls.jsx";
import { CatalogList, CatalogListCustomRow, CatalogListToolbar } from "../shared/catalog-list.jsx";
import { SelectorOptionIcon } from "../shared/select-picker.jsx";

const UI=()=>window.BlobUI;
const access=[{value:"none",label:"No Access"},{value:"read",label:"Read Only"},{value:"readWrite",label:"Read-Write"},{value:"createDelete",label:"Create-Delete"}];
const approval=[{value:"alwaysAsk",label:"Always Ask"},{value:"approveForMe",label:"Approve for me"},{value:"fullAccess",label:"Never Ask"}];

function SpaceRow({space,permission,disabled,onChange}) {
  const description=`${space.name}\n${space.id}`;
  const value=permission||{access:"none",filesEnabled:false,approval:"approveForMe"};
  return <CatalogListCustomRow className="codex-space">
    <div className="codex-space-top">{space.icon&&<span className="catalog-list-icon" aria-hidden="true"><SelectorOptionIcon icon={space.icon} /></span>}<span className="formula-view-name" title={description}>{space.name}</span></div>
    <div className="codex-space-bottom">
      <Switch id={`codex-files-${space.id}`} label="Upload/delete files" className="native-accent-control settings-row" inputAttributes={{"aria-label":`Upload/delete files for ${description}`}} checked={value.filesEnabled} disabled={disabled} onChange={checked=>onChange({...value,filesEnabled:checked})} />
      <PermissionPicker id={description} label="Access" options={access} value={value.access} disabled={disabled} onChange= {next=>onChange({...value,access:next})} />
      <PermissionPicker id={description} label="Approval" options={approval} value={value.approval} disabled={disabled} onChange={next=>onChange({...value,approval:next})} />
    </div>
  </CatalogListCustomRow>;
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
    <CatalogListToolbar prefix="codex" searchLabel="Search spaces" query={query} onQueryChange={setQuery}
      reloadLabel="Reload list" onReload={()=>controller.reloadSpaces()} locked={state.locked} busy={state.busy} showFilter={false} />
    <div id="codex-filter-control" hidden />
    <p className="input-hint" role="status">{state.message}</p>
    {state.message.includes("API key")&&<button type="button" className="formula-button" onClick={openApiKey}>Open Advanced Settings &gt; API key</button>}
    <div id="codex-space-list"><CatalogList sections={[{id:"spaces",items:spaces}]} emptyMessage={state.spaces.length>0&&!spaces.length?"No matching results.":""}
      renderItem={space=><SpaceRow key={space.id} space={space} permission={preferences.spaces[space.id]} disabled={state.locked} onChange={value=>controller.changePermission(space.id,value)} />} /></div>
  </>;
}
