import React, { useEffect, useRef, useState, useSyncExternalStore } from "react";
import { createPortal } from "react-dom";
import { ExpandableSearch } from "../shared/search.jsx";
import { SelectPicker } from "../shared/select-picker.jsx";
import { FilterIcon } from "../shared/catalog-icons.jsx";

const UI = () => window.BlobUI;
const accessOptions = [{value:"none",label:"No Access"},{value:"read",label:"Read Only"},{value:"readWrite",label:"Read-Write"},{value:"createDelete",label:"Create-Delete"}];
const approvalOptions = [{value:"alwaysAsk",label:"Always Ask"},{value:"approveForMe",label:"Approve for me"},{value:"fullAccess",label:"Never Ask"}];

function ReloadIcon(){return <svg viewBox="0 0 20 20" aria-hidden="true"><path d="M16 5.5v4h-4M15.8 9.2a6 6 0 1 0-.7 4" fill="none" /></svg>;}

function CatalogToolbar({prefix,label,onReload,locked,busy,categories,selectedCategories,onCategoriesChange,query,setQuery}) {
  return <div className="formula-toolbar">
    <ExpandableSearch id={`${prefix}-search`} label={label} placeholder={label} value={query} onChange={setQuery} disabled={locked} />
    {categories ? <SelectPicker id={`${prefix}-filter`} rootId={`${prefix}-filter-control`} menuId={`${prefix}-filter-options`}
      label="Filter source categories" title="Filter source categories" icon={<FilterIcon />} multiple
      pressed={selectedCategories.length !== categories.length} disabled={locked || busy}
      options={categories} value={selectedCategories} onChange={onCategoriesChange} /> : <div id={`${prefix}-filter-control`} hidden />}
    <button id={`${prefix}-reload`} type="button" className="neutral-icon-button" aria-label="Reload list" title="Reload list" disabled={locked||busy} aria-busy={String(busy)} onClick={onReload}><ReloadIcon /></button>
  </div>;
}

export function PermissionPicker({ id, label, options, value, disabled, onChange, dialogRef }) {
  return <SelectPicker label={`${label} for ${id}`} options={options} value={value} onChange={onChange}
    disabled={disabled} rootClassName="permission-picker" menuClassName="permission-picker-menu"
    portalRef={dialogRef} fixed={!dialogRef} />;
}

function SourceRow({source,selected,disabled,onToggle,onPermission,dialogRef}) {
  const permission=selected?.permission||{access:"none",filesEnabled:false,approval:"approveForMe"};
  const description=`${source.name} · ${({space:"Spaces",type:"Object types",collection:"Collections",query:"Queries"})[source.kind]||"Unknown source"} · ${source.space}`;
  return <section className="formula-view codex-space" data-selected={String(Boolean(selected))}>
    <div className="codex-space-top">
      <input type="checkbox" data-source-key={JSON.stringify([source.spaceId,source.kind,source.id])} checked={Boolean(selected)} disabled={disabled} aria-label={`Select ${description}`} onChange={event=>onToggle(event.currentTarget.checked)} />
      <span className="formula-view-name" title={source.name} onClick={event=>{const input=event.currentTarget.parentElement.querySelector('input[type="checkbox"]');input?.click();}}>{source.name}</span>
      <p className="input-hint project-source-metadata">{({space:"Spaces",type:"Object types",collection:"Collections",query:"Queries"})[source.kind]} · {source.space}</p>
    </div>
    <div className="codex-space-bottom">
      <label className="native-accent-control settings-row"><span>Upload/delete files</span><input type="checkbox" role="switch" aria-label={`Upload/delete files for ${description}`} checked={permission.filesEnabled} disabled={disabled||!selected} onChange={event=>onPermission({...permission,filesEnabled:event.currentTarget.checked})} /><span className="switch-track" aria-hidden="true" /></label>
      <PermissionPicker id={description} label="Access" options={accessOptions} value={permission.access} disabled={disabled||!selected} dialogRef={dialogRef} onChange={value=>onPermission({...permission,access:value})} />
      <PermissionPicker id={description} label="Approval" options={approvalOptions} value={permission.approval} disabled={disabled||!selected} dialogRef={dialogRef} onChange={value=>onPermission({...permission,approval:value})} />
    </div>
    {source.unavailable&&<p className="formula-view-warning">Unavailable source. Its saved selection is retained.</p>}
  </section>;
}

export function CodexProjects() {
  const controller=UI().ProjectPermissions;
  const state=useSyncExternalStore(controller.subscribe,controller.getSnapshot,controller.getSnapshot);
  const [query,setQuery]=useState(""),[sourceQuery,setSourceQuery]=useState(""),[categories,setCategories]=useState(controller.kinds.map(item=>item.value));
  const backdrop=useRef(null),dialog=useRef(null),close=useRef(null),content=useRef(null),modal=useRef(null);
  const projects=controller.projectCatalog();
  const projectMatches=project=>`${project.name} ${project.rootPaths.join(" ")}`.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase());
  const selected=new Map(controller.selectedSources().map(source=>[controller.identity(source),source]));
  const sources=controller.sourceCatalog().filter(source=>categories.includes(source.kind)&&`${source.name} ${source.space} ${source.kind}`.toLocaleLowerCase().includes(sourceQuery.trim().toLocaleLowerCase()));
  useEffect(()=>{if(!backdrop.current||!dialog.current)return;modal.current=UI().createModalController({backdrop:backdrop.current,dialog:dialog.current,initialFocus:()=>close.current,fallbackFocus:()=>document.getElementById("codex-tab-2"),inertTargets:[document.getElementById("settings-shell"),document.getElementById("title-drag-region")].filter(Boolean),onClose:()=>controller.close()});return()=>{modal.current?.destroy();modal.current=null;};},[controller]);
  useEffect(()=>{if(state.modal)modal.current?.open();else if(modal.current?.isOpen())modal.current.close();},[state.modal]);
  useEffect(()=>{if(!state.modal||!content.current)return;const scrollbar=UI().Scrollbar.attach(content.current);return()=>scrollbar.destroy();},[state.modal]);
  return <>
    <CatalogToolbar prefix="projects" label="Search projects and folders" locked={state.locked} busy={state.busy} query={query} setQuery={setQuery} onReload={()=>controller.reloadProjects()} />
    <p className="input-hint" role="status">{state.projectStatus}</p>
    <div id="codex-project-list">{projects.map(project=><section className="formula-view" key={project.id} hidden={!projectMatches(project)}><button type="button" className="formula-view-row" disabled={state.locked} title={project.name} onClick={()=>controller.selectProject(project)}><span className="formula-view-name">{project.name}</span>{project.unavailable&&<span className="formula-view-warning">Unavailable project. Saved selections are retained.</span>}</button></section>)}{state.scanned&&!projects.some(projectMatches)&&<p className="input-hint">{query?"No matching results.":"No local Codex projects found."}</p>}</div>
    {createPortal(<div id="project-source-backdrop" className="confirm-backdrop" hidden ref={backdrop}>
      <section id="project-source-dialog" className="confirm-dialog calendar-rule-dialog project-source-dialog" role="dialog" aria-modal="true" aria-labelledby="project-source-title" tabIndex={-1} ref={dialog}>
        <header className="calendar-modal-header"><h2 id="project-source-title">{state.active?.name||"Project sources"}</h2><button ref={close} type="button" className="neutral-icon-button calendar-modal-close" aria-label="Close project sources" onClick={()=>modal.current?.close()}>×</button></header>
        <CatalogToolbar prefix="project-sources" label="Search sources" locked={state.locked} busy={state.sourceBusy} query={sourceQuery} setQuery={setSourceQuery} categories={controller.kinds} selectedCategories={categories} onCategoriesChange={setCategories} onReload={()=>controller.reloadSources()} />
        <div className="calendar-modal-content project-source-content" ref={content}>
          <p className="input-hint" role="status">{state.sourceStatus}</p>
          <div id="project-source-list">{sources.map(source=>{const key=controller.identity(source),selectedSource=selected.get(key);return <SourceRow key={key} source={source} selected={selectedSource} disabled={state.locked} dialogRef={dialog} onToggle={checked=>controller.toggleSource(source,checked)} onPermission={permission=>controller.setPermission(source,permission)} />;})}{state.sourcesScanned&&!sources.length&&<p className="input-hint">No matching sources.</p>}</div>
        </div>
      </section>
    </div>, document.body)}
  </>;
}
