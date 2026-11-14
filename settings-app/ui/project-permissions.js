(function (root) {
  'use strict';
  const UI = root.BlobUI;
  const copy = value => JSON.parse(JSON.stringify(value));
  const kinds = [{value:'space',label:'Spaces'},{value:'type',label:'Object types'},{value:'collection',label:'Collections'},{value:'query',label:'Queries'}];
  const kindName = kind => kinds.find(item => item.value === kind)?.label || 'Unknown source';
  const identity = source => JSON.stringify([source.spaceId,source.kind,source.id]);
  const fallback = () => ({access:'none',filesEnabled:false,approval:'approveForMe'});

  function cloudCoordinates(key) {
    if (typeof key !== 'string' || !key.startsWith('cloud:')) return null;
    const separator = key.indexOf(':', 6);
    if (separator < 0) return null;
    const hostId = key.slice(6, separator);
    const id = key.slice(separator + 1);
    return hostId && id ? {hostId, id} : null;
  }

  function projectKey(project) {
    return project?.kind === 'cloud' ? `cloud:${project.hostId || ''}:${project.id || ''}` : String(project?.id || '');
  }

  function normalize(value) {
    const result = Object.create(null);
    if (!value || typeof value !== 'object' || Array.isArray(value)) return result;
    for (const [key, entry] of Object.entries(value)) {
      const coordinates = cloudCoordinates(key);
      const isCloud = entry?.kind === 'cloud' || Boolean(coordinates);
      const sources = (Array.isArray(entry?.sources) ? entry.sources : entry?.sources == null ? [] : [null]).map(source=>(
        {spaceId:typeof source?.spaceId==='string'?source.spaceId:'',kind:typeof source?.kind==='string'?source.kind:'',id:typeof source?.id==='string'?source.id:'',
          name:typeof source?.name==='string'?source.name:'Unavailable source',space:typeof source?.space==='string'?source.space:'',
          permission:{access:['none','read','readWrite','createDelete'].includes(source?.permission?.access)?source.permission.access:'none',filesEnabled:source?.permission?.filesEnabled===true,approval:['alwaysAsk','approveForMe','fullAccess'].includes(source?.permission?.approval)?source.permission.approval:'approveForMe'}}));
      result[key] = {
        id: typeof entry?.id === 'string' ? entry.id : coordinates?.id || key,
        kind: isCloud ? 'cloud' : 'local',
        hostId: typeof entry?.hostId === 'string' ? entry.hostId : coordinates?.hostId || '',
        computerName: typeof entry?.computerName === 'string' ? entry.computerName : '',
        name: typeof entry?.name === 'string' ? entry.name : key,
        rootPaths:Array.isArray(entry?.rootPaths) ? entry.rootPaths.filter(path=>typeof path==='string') : [],
        sources,
      };
    }
    return result;
  }

  function normalizeCatalogProject(project) {
    if (!project || typeof project.id !== 'string' || typeof project.name !== 'string' || !Array.isArray(project.rootPaths)) {
      throw new Error('Unsupported Codex project metadata.');
    }
    const kind = project.kind === 'cloud' ? 'cloud' : 'local';
    if (project.kind !== undefined && project.kind !== 'local' && project.kind !== 'cloud') {
      throw new Error('Unsupported Codex project metadata.');
    }
    if (kind === 'cloud' && (typeof project.hostId !== 'string' || !project.hostId)) {
      throw new Error('Unsupported Codex cloud project metadata.');
    }
    if (kind === 'cloud' && project.computerName !== undefined && typeof project.computerName !== 'string') {
      throw new Error('Unsupported Codex cloud project metadata.');
    }
    return {
      ...project,
      kind,
      hostId: kind === 'cloud' ? project.hostId : '',
      computerName: kind === 'cloud' ? project.computerName || '' : '',
      rootPaths: project.rootPaths.filter(path => typeof path === 'string'),
    };
  }

  function savedProject(key, saved) {
    const coordinates = cloudCoordinates(key);
    const isCloud = saved?.kind === 'cloud' || Boolean(coordinates);
    return {
      id: typeof saved?.id === 'string' ? saved.id : coordinates?.id || key,
      kind: isCloud ? 'cloud' : 'local',
      hostId: typeof saved?.hostId === 'string' ? saved.hostId : coordinates?.hostId || '',
      computerName: typeof saved?.computerName === 'string' ? saved.computerName : '',
      name: typeof saved?.name === 'string' ? saved.name : key,
      rootPaths: Array.isArray(saved?.rootPaths) ? saved.rootPaths : [],
      unavailable: false,
    };
  }

  function createController() {
    let api, locked=true, projects=[], cloudProjects=[], cloudCatalogLoaded=false, sources=[], scanned=false, sourcesScanned=false;
    let busy=false, sourceBusy=false, active=null, openRequested=false, pendingCloudRefresh=false;
    let destroyed=false, cloudUnlisten=null;
    let snapshot={projects:[],sources:[],scanned:false,sourcesScanned:false,busy:false,sourceBusy:false,locked:true,active:null,modal:false,projectStatus:'',sourceStatus:''};
    const listeners=new Set();
    const subscribe=listener=>{listeners.add(listener);return()=>listeners.delete(listener);};
    const getSnapshot=()=>snapshot;
    function publish(patch={}) { snapshot={...snapshot,...patch}; listeners.forEach(listener=>listener()); }
    function configuration() { return api.normalize(api.read()); }
    function selectedSources() { return active && api ? configuration().projects[projectKey(active)]?.sources || [] : []; }

    function setLocked(value) {
      const wasLocked=locked;
      locked=value===true;
      publish({locked});
      if (wasLocked && !locked) {
        if (pendingCloudRefresh && !busy) {
          pendingCloudRefresh=false;
          void reloadProjects({replaceCloud:true});
        } else if (openRequested && !scanned && !busy) {
          void reloadProjects();
        }
      }
    }

    function saveSources(nextSources) {
      if (!active || locked) return;
      const key=projectKey(active);
      const next=configuration(), baseline=api.normalize(api.readSaved()).projects[key];
      const ordered=entries=>entries.slice().sort((a,b)=>identity(a).localeCompare(identity(b)));
      if(JSON.stringify(ordered(nextSources))===JSON.stringify(ordered(baseline?.sources||[]))){if(baseline)next.projects[key]=copy(baseline);else delete next.projects[key];}
      else next.projects[key]={id:active.id,kind:active.kind,hostId:active.hostId||'',computerName:active.computerName||'',name:active.name,rootPaths:active.rootPaths.slice(),sources:nextSources};
      api.onChange(next);publish({});
    }

    function catalogSources() {
      if(!active)return [];
      const selected=selectedSources(), catalog=sources.map(source=>({...source}));
      for(const source of selected) if(!catalog.some(row=>identity(row)===identity(source))) catalog.push({...source,unavailable:sourcesScanned});
      return catalog.sort((a,b)=>Number(selected.some(row=>identity(row)===identity(b)))-Number(selected.some(row=>identity(row)===identity(a)))||a.name.localeCompare(b.name)||identity(a).localeCompare(identity(b)));
    }

    function catalogHasProject(catalog, project) {
      const key=projectKey(project);
      return catalog.some(entry=>projectKey(entry)===key);
    }

    function catalogProjects() {
      const catalog=projects.map(project=>({...project}));
      if(api)for(const[key,saved]of Object.entries(configuration().projects))if(!catalogHasProject(catalog,savedProject(key,saved))){
        const project=savedProject(key,saved);
        const known=project.kind==='cloud'?cloudCatalogLoaded:scanned;
        const available=project.kind==='cloud'?cloudProjects:projects;
        catalog.push({...project,unavailable:known&&!catalogHasProject(available,project)});
      }
      return catalog.sort((a,b)=>a.name.localeCompare(b.name)||projectKey(a).localeCompare(projectKey(b)));
    }

    async function reloadProjects({replaceCloud=false}={}) {
      if(locked||destroyed)return;
      if(busy){if(replaceCloud)pendingCloudRefresh=true;return;}
      busy=true;publish({busy:true,projectStatus:'Loading projects…'});
      try {
        const result=await api.invoke('discover_codex_projects');
        if(!Array.isArray(result))throw new Error('Unsupported Codex project metadata.');
        const normalized=result.map(normalizeCatalogProject);
        const local=normalized.filter(project=>project.kind==='local');
        const cloud=normalized.filter(project=>project.kind==='cloud');
        if(replaceCloud||!cloudCatalogLoaded||cloud.length){cloudProjects=cloud;cloudCatalogLoaded=true;}
        projects=local.concat(cloudProjects);
        scanned=true;
        publish({projects,scanned,projectStatus:''});
        if(active){
          active=projects.find(project=>projectKey(project)===projectKey(active))||{...active,unavailable:true};
          publish({active});
        }
      } catch(error) { publish({projectStatus:String(error.message||error)}); }
      finally {
        busy=false;publish({busy:false});
        if(pendingCloudRefresh&&!locked&&!destroyed){pendingCloudRefresh=false;void reloadProjects({replaceCloud:true});}
      }
    }

    async function reloadSources(forceRefresh=false) {
      if(sourceBusy||locked||destroyed)return;sourceBusy=true;publish({sourceBusy:true,sourceStatus:'Loading sources…'});
      try {const result=await api.invoke('discover_codex_sources',{forceRefresh});
        if(!Array.isArray(result)||result.some(source=>!source.id||!source.spaceId||!kinds.some(kind=>kind.value===source.kind)||typeof source.name!=='string'))throw Error('Source discovery returned an invalid response.');
        sources=result;sourcesScanned=true;publish({sources,sourcesScanned,sourceStatus:''});
      } catch(error){publish({sourceStatus:String(error.message||error).replace(' to use Kanban formulas',' to discover sources')});}
      finally{sourceBusy=false;publish({sourceBusy:false});}
    }

    function onCloudProjectsChanged() {
      pendingCloudRefresh=true;
      if(!locked&&!busy){pendingCloudRefresh=false;void reloadProjects({replaceCloud:true});}
    }

    return {
      subscribe,getSnapshot,kinds,
      init(options){
        api=options;destroyed=false;UI.AppReload?.register(()=>reloadSources(true));
        if(typeof api.listen==='function'){
          Promise.resolve(api.listen('anyblob-cloud-projects-changed',onCloudProjectsChanged)).then(unlisten=>{
            if(typeof unlisten!=='function')return;
            if(destroyed)unlisten();else cloudUnlisten=unlisten;
          }).catch(()=>{});
        }
      },
      normalize,identity,kindName,projectKey,
      open(){openRequested=true;if(!scanned&&!busy&&!locked)void reloadProjects();},
      reloadProjects(){return reloadProjects();},reloadSources(){return reloadSources(true);},
      setLocked,
      selectProject(project){if(locked)return;active={...project,key:projectKey(project)};publish({active,modal:true,sourceStatus:''});if(!sourcesScanned&&!sourceBusy)void reloadSources();},
      close(){publish({modal:false});},
      projectCatalog:catalogProjects,
      sourceCatalog:catalogSources,selectedSources,
      toggleSource(source,checked){if(!active||locked)return;const key=identity(source),entries=selectedSources().filter(row=>identity(row)!==key);if(checked)entries.push({...source,permission:copy(configuration().spaces[source.spaceId]||fallback())});saveSources(entries);},
      setPermission(source,permission){if(!active||locked)return;const key=identity(source);saveSources(selectedSources().map(row=>identity(row)===key?{...row,permission}:row));},
      render(){publish({});},
      destroy(){destroyed=true;pendingCloudRefresh=false;cloudUnlisten?.();cloudUnlisten=null;listeners.clear();}
    };
  }
  const controller=createController();
  UI.ProjectPermissions={...controller,createController,normalize,identity,projectKey,kinds};
})(window);
