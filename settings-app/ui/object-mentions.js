(function (root) {
  'use strict';
  const UI=root.BlobUI;
  const key=(kind,row)=>kind==='spaces'?row.id:JSON.stringify([row.spaceId,row.id]);
  function createController(){
    let api,normalize,spaces=[],types=[],epoch=0,pending=false,loaded=false,status='';
    let snapshot={spaces:[],types:[],status:'',loaded:false,pending:false,selections:{spaces:{all:true,items:[]},types:{all:true,items:[]},filterIncompleteTasks:false}};
    const listeners=new Set();
    const subscribe=listener=>{listeners.add(listener);return()=>listeners.delete(listener);};
    const getSnapshot=()=>snapshot;
    function publish(patch={}){snapshot={...snapshot,...patch};listeners.forEach(listener=>listener());}
    function filters(){return api?normalize(api.read()).objectMentions:{spaces:{all:true,items:[]},types:{all:true,items:[]},filterIncompleteTasks:false};}
    function options(kind){
      const saved=filters(),selected=saved[kind],entries=new Map();
      const active=sid=>saved.spaces.all||saved.spaces.items.some(space=>space.id===sid);
      const available=kind==='spaces'?spaces:types.filter(type=>active(type.spaceId)&&spaces.some(space=>space.id===type.spaceId));
      for(const row of available)entries.set(key(kind,row),{row,available:true});
      for(const row of selected.items)if(!entries.has(key(kind,row)))entries.set(key(kind,row),{row,available:false});
      return [{value:'*',label:'All'},...[...entries].map(([value,{row,available}])=>({value,row,label:kind==='spaces'?row.name:row.name+' · '+row.space,description:kind==='types'&&!active(row.spaceId)?'Inactive outside filtered spaces':available?'':'Unavailable saved selection'})).sort((a,b)=>a.label.localeCompare(b.label)||a.value.localeCompare(b.value))];
    }
    function change(kind,values){
      if(!api)return;
      const next=normalize(api.read()),before=next.objectMentions[kind],all=values.includes('*')&&!before.all,selected=values.filter(value=>value!=='*');
      next.objectMentions[kind]={all,items:all?[]:options(kind).filter(option=>selected.includes(option.value)).map(option=>option.row)};
      api.onChange(next);refresh();if(kind==='spaces')void discoverTypes();
    }
    function setIncompleteTasks(checked){if(!api)return;const next=normalize(api.read());next.objectMentions.filterIncompleteTasks=checked===true;api.onChange(next);refresh();}
    function refresh(){const selected=filters();publish({spaces:options('spaces'),types:options('types'),selections:selected,filterIncompleteTasks:selected.filterIncompleteTasks});}
    async function discoverTypes(forceRefresh=false){
      const current=++epoch,saved=filters(),ids=spaces.filter(space=>forceRefresh||saved.spaces.all||saved.spaces.items.some(value=>value.id===space.id)).map(space=>space.id);
      if(!ids.length){status='';publish({status});refresh();return;}
      status='Loading object types…';publish({status});
      try{const result=await api.invoke('discover_anyblob_types',{spaceIds:ids,forceRefresh});if(current!==epoch)return;if(!Array.isArray(result?.types)||!Array.isArray(result?.errors))throw Error('Type discovery returned an invalid response.');const failed=new Set(result.errors.map(error=>error.spaceId));types=types.filter(type=>!ids.includes(type.spaceId)||failed.has(type.spaceId));const merged=new Map(types.map(type=>[key('types',type),type]));for(const type of result.types)merged.set(key('types',type),type);types=[...merged.values()];status=result.errors.map(error=>error.error).join(' ');}
      catch(error){if(current===epoch)status=String(error.message||error);}
      finally{if(current===epoch){publish({types,status});refresh();}}
    }
    async function discover(forceRefresh=false){
      if(pending||!api)return;pending=true;status='Loading spaces…';publish({status,pending});
      try{const result=await api.invoke('discover_codex_spaces',{forceRefresh});if(!Array.isArray(result))throw Error('Space discovery returned an invalid response.');spaces=result;loaded=true;status='';publish({spaces,loaded,status});refresh();await discoverTypes(forceRefresh);}
      catch(error){status=String(error.message||error);publish({status});}
      finally{pending=false;publish({pending});refresh();}
    }
    return {subscribe,getSnapshot,init(options,normalizer){api=options;normalize=normalizer;UI.AppReload?.register(()=>discover(true));refresh();},options,change,setIncomplete:setIncompleteTasks,refresh,discover(){return discover(false);},show(){refresh();if(!loaded&&!pending)void discover();},discoverTypes,destroy(){epoch++;listeners.clear();}};
  }
  const controller=createController();
  UI.ObjectMentions={...controller,createController};
})(window);
