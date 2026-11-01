(function (root) {
  'use strict';
  const UI = root.BlobUI, C = UI.Components, doc = root.document, make = UI.CatalogBrowser.node;
  const copy = value => JSON.parse(JSON.stringify(value));
  const kinds = [{value:'space',label:'Spaces'},{value:'type',label:'Object types'},{value:'collection',label:'Collections'},{value:'query',label:'Queries'}];
  const kindName = kind => kinds.find(item => item.value === kind)?.label || 'Unknown source';
  const identity = source => JSON.stringify([source.spaceId,source.kind,source.id]);
  const fallback = () => ({access:'none',filesEnabled:false,approval:'approveForMe'});
  const warning = 'Project access is not available yet. Saving any source selection pauses all Anyblob reads until Codex supplies verified chat identity. Remove every project source selection to use global permissions.';
  function normalize(value) {
    const result = Object.create(null);
    if (!value || typeof value !== 'object' || Array.isArray(value)) return result;
    for (const [id, entry] of Object.entries(value)) {
      const sources = (Array.isArray(entry?.sources) ? entry.sources : entry?.sources == null ? [] : [null]).map(source=>({
        spaceId:typeof source?.spaceId==='string'?source.spaceId:'',kind:typeof source?.kind==='string'?source.kind:'',id:typeof source?.id==='string'?source.id:'',
        name:typeof source?.name==='string'?source.name:'Unavailable source',space:typeof source?.space==='string'?source.space:'',
        permission:{access:['none','read','readWrite','createDelete'].includes(source?.permission?.access)?source.permission.access:'none',filesEnabled:source?.permission?.filesEnabled===true,approval:['alwaysAsk','approveForMe','fullAccess'].includes(source?.permission?.approval)?source.permission.approval:'approveForMe'}
      }));
      result[id] = {name:typeof entry?.name === 'string' ? entry.name : id,rootPaths:Array.isArray(entry?.rootPaths) ? entry.rootPaths.filter(path=>typeof path==='string') : [],sources};
    }
    return result;
  }
  let api, locked = true, projects = [], sources = [], scanned = false, sourcesScanned = false, busy = false, sourceBusy = false;
  let list, status, empty, browser, records = [], sourceList, sourceStatus, sourceEmpty, sourceBrowser, sourceRecords = [], rows = [];
  let active, shell, modal, heading, close, scroll;
  function configuration() { return api.normalize(api.read()); }
  function saveSources(nextSources) {
    const next = configuration();
    next.projects[active.id] = {name:active.name,rootPaths:active.rootPaths.slice(),sources:nextSources};
    api.onChange(next);
  }
  function renderSources(focusKey) {
    if (!sourceList || !active) return;
    const focusRole = doc.activeElement?.closest('[data-source-key]')?.dataset.sourceKey;
    rows.forEach(row=>row.destroy()); rows=[]; sourceList.replaceChildren(); sourceRecords=[];
    const config = configuration(), selected = config.projects[active.id]?.sources || [];
    const catalog = sources.map(source=>({...source}));
    for (const source of selected) if (!catalog.some(row=>identity(row)===identity(source))) catalog.push({...source,unavailable:sourcesScanned});
    catalog.sort((a,b)=>Number(selected.some(row=>identity(row)===identity(b)))-Number(selected.some(row=>identity(row)===identity(a))) || a.name.localeCompare(b.name) || identity(a).localeCompare(identity(b)));
    for (const source of catalog) {
      const key = identity(source), saved = selected.find(row=>identity(row)===key);
      const selection = doc.createElement('input'); selection.type='checkbox';selection.checked=!!saved;selection.disabled=locked;
      selection.dataset.sourceKey=key;selection.setAttribute('aria-label','Select '+source.name+' · '+kindName(source.kind)+' · '+source.space);
      selection.addEventListener('change',()=>{
        if (locked) return;
        const current = configuration(), entries = current.projects[active.id]?.sources || [];
        const next = entries.filter(row=>identity(row)!==key);
        if (selection.checked) next.push({...source,permission:copy(current.spaces[source.spaceId] || fallback())});
        saveSources(next);renderSources(key);renderProjects();
      });
      const row = api.permissionRow({view:source,permission:copy(saved?.permission || fallback()),disabled:locked || !saved,selection,dialog:shell.dialog,scrollContainer:sourceList.parentElement,
        metadata:kindName(source.kind)+' · '+source.space+' · '+source.spaceId+' · '+source.id,
        onChange(permission) {
          const entries=configuration().projects[active.id]?.sources || [];
          saveSources(entries.map(row=>identity(row)===key ? {...row,permission} : row));
        }});
      row.item.dataset.selected=String(!!saved);
      if (source.unavailable) row.item.append(make('p','formula-view-warning','Unavailable source. Its saved selection is retained.'));
      sourceList.append(row.item);rows.push(row);sourceRecords.push({item:row.item,view:{...source,target:{spaceId:source.spaceId}}});
    }
    if (!catalog.length) sourceList.append(make('p','input-hint',sourceBusy?'Loading sources…':sourcesScanned?'No accessible sources found.':'Reload to discover sources.'));
    sourceBrowser.refresh(); scroll?.update();
    if (focusKey || focusRole) [...sourceList.querySelectorAll('[data-source-key]')].find(input=>input.dataset.sourceKey===(focusKey||focusRole))?.focus({preventScroll:true});
  }
  function renderProjects() {
    if (!list) return;
    C.destroyTree(list);list.replaceChildren();records=[];
    const catalog=projects.map(project=>({...project}));
    if (scanned) for (const [id, saved] of Object.entries(configuration().projects)) if (!catalog.some(project=>project.id===id)) catalog.push({id,name:saved.name,rootPaths:saved.rootPaths,unavailable:true});
    catalog.sort((a,b)=>a.name.localeCompare(b.name)||a.id.localeCompare(b.id));
    for (const project of catalog) {
      const row=UI.CatalogBrowser.row({name:project.name,metadata:project.rootPaths.join(' · '),warning:project.unavailable?'Unavailable project. Saved selections are retained.':'',onSelect() {
        if (locked) return;
        active=project;heading.textContent=project.name;renderSources();modal.open();
        if (!sourcesScanned && !sourceBusy) void reloadSources();
      }});
      row.row.disabled=locked;row.row.title=project.name+'\n'+project.id+'\n'+project.rootPaths.join('\n');
      list.append(row.item);records.push({...row,view:{name:project.name,source:project.rootPaths.join(' '),target:{spaceId:project.id}}});
    }
    if (scanned && !catalog.length) list.append(make('p','input-hint','No local Codex projects found.'));
    browser.refresh();
  }
  async function reloadProjects() {
    if (busy || locked) return;
    busy=true;browser.setLocked(locked,true);status.textContent='Loading projects…';
    try {
      const result=await api.invoke('discover_codex_projects');
      if (!Array.isArray(result) || result.some(project=>typeof project.id!=='string'||typeof project.name!=='string'||!Array.isArray(project.rootPaths))) throw Error('Unsupported Codex project metadata.');
      projects=result;scanned=true;status.textContent='';renderProjects();
      if (active) {active=projects.find(project=>project.id===active.id)||{...active,unavailable:true};heading.textContent=active.name;}
    } catch(error) {UI.CatalogBrowser.status(status,String(error.message||error));}
    finally {busy=false;browser.setLocked(locked);}
  }
  async function reloadSources() {
    if (sourceBusy || locked) return;
    sourceBusy=true;sourceBrowser.setLocked(locked,true);sourceStatus.textContent='Loading sources…';
    try {
      const result=await api.invoke('discover_codex_sources');
      if (!Array.isArray(result) || result.some(source=>!source.id||!source.spaceId||!kinds.some(kind=>kind.value===source.kind)||typeof source.name!=='string')) throw Error('Source discovery returned an invalid response.');
      sources=result;sourcesScanned=true;sourceStatus.textContent='';renderSources();
    } catch(error) {UI.CatalogBrowser.status(sourceStatus,String(error.message||error).replace(' to use Kanban formulas',' to discover sources'));}
    finally {sourceBusy=false;sourceBrowser.setLocked(locked);}
  }
  function init(options) {
    api=options;
    const toolbar=UI.CatalogBrowser.toolbar('projects','Search projects and folders');toolbar.querySelector('#projects-filter-control').hidden=true;
    status=make('p','input-hint');status.setAttribute('role','status');list=make('div');list.id='codex-project-list';empty=make('p','input-hint');empty.hidden=true;
    const notice=make('p','input-hint project-permission-notice',warning);
    api.panel.append(notice,toolbar,status,list,empty);
    browser=UI.CatalogBrowser.attach({prefix:'projects',toolbar,records:()=>records,empty,onReload:reloadProjects});
    heading=make('h2');heading.id='project-source-title';
    close=C.IconButton({accessibleLabel:'Close project sources',content:['×'],attributes:{class:'neutral-icon-button calendar-modal-close'},onClick:()=>modal.close()}).element;
    const header=make('div','calendar-modal-header');header.append(heading,close);
    const sourceToolbar=UI.CatalogBrowser.toolbar('project-sources','Search sources');
    sourceStatus=make('p','input-hint');sourceStatus.setAttribute('role','status');sourceList=make('div');sourceList.id='project-source-list';sourceEmpty=make('p','input-hint');sourceEmpty.hidden=true;
    const content=make('div','calendar-modal-content project-source-content');content.append(sourceStatus,sourceList,sourceEmpty);
    shell=C.DialogShell({attributes:{id:'project-source-backdrop'},dialogAttributes:{id:'project-source-dialog',class:'confirm-dialog calendar-rule-dialog project-source-dialog','aria-labelledby':heading.id},content:[header,make('p','input-hint project-permission-notice',warning),sourceToolbar,content]});
    doc.body.append(shell.element);
    sourceBrowser=UI.CatalogBrowser.attach({prefix:'project-sources',toolbar:sourceToolbar,records:()=>sourceRecords,empty:sourceEmpty,onReload:reloadSources,categories:kinds});
    scroll=UI.Scrollbar.attach(content);
    modal=shell.attach({initialFocus:close,fallbackFocus:()=>doc.getElementById('codex-tab-2'),inertTargets:[doc.getElementById('settings-shell'),doc.getElementById('title-drag-region')].filter(Boolean)});
    renderProjects();
  }
  UI.ProjectPermissions={normalize,identity,init,open(){if(!scanned&&!busy)void reloadProjects();},render(){if(!api)return;renderProjects();if(active)renderSources();},setLocked(value){locked=value;browser?.setLocked(value,busy);sourceBrowser?.setLocked(value,sourceBusy);if(api){renderProjects();if(active)renderSources();}}};
})(window);
