(function (root) {
  'use strict';
  const UI = root.BlobUI;
  const copy = value => JSON.parse(JSON.stringify(value));
  const kinds = [{value:'space',label:'Spaces'},{value:'type',label:'Object types'},{value:'collection',label:'Collections'},{value:'query',label:'Queries'}];
  const kindName = kind => kinds.find(item => item.value === kind)?.label || 'Unknown source';
  const identity = source => JSON.stringify([source.spaceId,source.kind,source.id]);
  const fallback = () => ({access:'none',filesEnabled:false,approval:'approveForMe'});
  function projectKey(project) {
    return String(project?.id || '');
  }

  function normalize(value) {
    const result = Object.create(null);
    if (!value || typeof value !== 'object' || Array.isArray(value)) return result;
    for (const [key, entry] of Object.entries(value)) {
      if (key.startsWith('cloud:') || ['cloud', 'remote'].includes(entry?.kind) || entry?.hostId || entry?.computerName) continue;
      const sources = (Array.isArray(entry?.sources) ? entry.sources : entry?.sources == null ? [] : [null]).map(source=>(
        {spaceId:typeof source?.spaceId==='string'?source.spaceId:'',kind:typeof source?.kind==='string'?source.kind:'',id:typeof source?.id==='string'?source.id:'',
          name:typeof source?.name==='string'?source.name:'Unavailable source',space:typeof source?.space==='string'?source.space:'',
          permission:{access:['none','read','readWrite','createDelete'].includes(source?.permission?.access)?source.permission.access:'none',filesEnabled:source?.permission?.filesEnabled===true,approval:['alwaysAsk','approveForMe','fullAccess'].includes(source?.permission?.approval)?source.permission.approval:'approveForMe'}}));
      result[key] = {
        id: typeof entry?.id === 'string' ? entry.id : key,
        kind: 'local',
        name: typeof entry?.name === 'string' ? entry.name : key,
        rootPaths:Array.isArray(entry?.rootPaths) ? entry.rootPaths.filter(path=>typeof path==='string') : [],
        sources,
      };
    }
    return result;
  }

  function normalizeCatalogProject(project) {
    if (!project || typeof project.id !== 'string' || typeof project.name !== 'string' || !Array.isArray(project.rootPaths)) {
      throw new Error('Unsupported local Anyblob project metadata.');
    }
    if (project.kind !== undefined && project.kind !== 'local') {
      throw new Error('Unsupported local Anyblob project metadata.');
    }
    return {
      ...project,
      kind: 'local',
      rootPaths: project.rootPaths.filter(path => typeof path === 'string'),
    };
  }

  function savedProject(key, saved) {
    return {
      id: typeof saved?.id === 'string' ? saved.id : key,
      kind: 'local',
      name: typeof saved?.name === 'string' ? saved.name : key,
      rootPaths: Array.isArray(saved?.rootPaths) ? saved.rootPaths : [],
      unavailable: false,
    };
  }

  function createController() {
    let api, locked=true, projects=[], sources=[], scanned=false, sourcesScanned=false;
    let busy=false, sourceBusy=false, active=null, openRequested=false;
    let destroyed=false;
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
      if (wasLocked && !locked && openRequested && !scanned && !busy) void reloadProjects();
    }

    function saveSources(nextSources) {
      if (!active || locked) return;
      const key=projectKey(active);
      const next=configuration(), baseline=api.normalize(api.readSaved()).projects[key];
      const ordered=entries=>entries.slice().sort((a,b)=>identity(a).localeCompare(identity(b)));
      if(JSON.stringify(ordered(nextSources))===JSON.stringify(ordered(baseline?.sources||[]))){if(baseline)next.projects[key]=copy(baseline);else delete next.projects[key];}
      else next.projects[key]={id:active.id,kind:'local',name:active.name,rootPaths:active.rootPaths.slice(),sources:nextSources};
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
        catalog.push({...project,unavailable:scanned&&!catalogHasProject(projects,project)});
      }
      return catalog.sort((a,b)=>a.name.localeCompare(b.name)||projectKey(a).localeCompare(projectKey(b)));
    }

    async function reloadProjects() {
      if(locked||destroyed)return;
      if(busy)return;
      busy=true;publish({busy:true,projectStatus:'Loading projects…'});
      try {
        const result=await api.invoke('discover_codex_projects');
        if(!Array.isArray(result))throw new Error('Unsupported Codex project metadata.');
        projects=result.map(normalizeCatalogProject);
        scanned=true;
        publish({projects,scanned,projectStatus:''});
        if(active){
          active=projects.find(project=>projectKey(project)===projectKey(active))||{...active,unavailable:true};
          publish({active});
        }
      } catch(error) { publish({projectStatus:String(error.message||error)}); }
      finally {
        busy=false;publish({busy:false});
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

    return {
      subscribe,getSnapshot,kinds,
      init(options){
        api=options;destroyed=false;UI.AppReload?.register(()=>reloadSources(true));
      },
      normalize,identity,kindName,projectKey,
      open(){openRequested=true;if(!scanned&&!busy&&!locked)void reloadProjects();},
      reloadProjects(){return reloadProjects();},reloadSources(){return reloadSources(true);},
      setLocked,
      selectProject(project){if(locked)return;active={...project,key:projectKey(project)};publish({active,modal:true,sourceStatus:''});if(!sourcesScanned&&!sourceBusy)void reloadSources();},
      close(){publish({modal:false});},
      projectCatalog:catalogProjects,
      deleteUnavailableProjects(){
        if(locked||!api)return;
        const unavailable=catalogProjects().filter(project=>project.unavailable).map(projectKey);
        if(!unavailable.length)return;
        const next=configuration();
        for(const key of unavailable)delete next.projects[key];
        api.onChange(next);publish({});
      },
      sourceCatalog:catalogSources,selectedSources,
      toggleSource(source,checked){if(!active||locked)return;const key=identity(source),entries=selectedSources().filter(row=>identity(row)!==key);if(checked)entries.push({...source,permission:copy(configuration().spaces[source.spaceId]||fallback())});saveSources(entries);},
      setPermission(source,permission){if(!active||locked)return;const key=identity(source);saveSources(selectedSources().map(row=>identity(row)===key?{...row,permission}:row));},
      render(){publish({});},
      destroy(){destroyed=true;listeners.clear();}
    };
  }
  const controller=createController();
  UI.ProjectPermissions={...controller,createController,normalize,identity,projectKey,kinds};
})(window);
