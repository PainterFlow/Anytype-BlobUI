(function (root) {
  'use strict';
  const UI=root.BlobUI;
  const typeKey=row=>JSON.stringify([row.spaceId,row.id]);
  const wildcardKey=sid=>JSON.stringify([sid,'*']);
  function createController(){
    let api,normalize,spaces=[],types=[],epoch=0,pending=false,loaded=false,status='';
    const empty={spaces:{all:true,items:[]},types:{all:true,items:[]},filterIncompleteTasks:false};
    let snapshot={sections:[],status:'',loaded:false,pending:false,selections:empty};
    const listeners=new Set();
    const subscribe=listener=>{listeners.add(listener);return()=>listeners.delete(listener);};
    const getSnapshot=()=>snapshot;
    function publish(patch={}){snapshot={...snapshot,...patch};listeners.forEach(listener=>listener());}
    function filters(){return api?normalize(api.read()).objectMentions:empty;}
    function readable(id){const permission=api?normalize(api.read()).spaces[id]:null;return permission?permission.access!=='none':false;}
    function mergedSpaces(saved){
      const merged=new Map();
      for(const space of spaces)if(readable(space.id))merged.set(space.id,{id:space.id,name:space.name,icon:space.icon});
      for(const space of saved.spaces.items)if(!merged.has(space.id))merged.set(space.id,{id:space.id,name:space.name});
      for(const type of saved.types.items)if(!merged.has(type.spaceId))merged.set(type.spaceId,{id:type.spaceId,name:type.space});
      return [...merged.values()].sort((a,b)=>a.name.localeCompare(b.name)||a.id.localeCompare(b.id));
    }
    function buildSections(saved){
      const known=new Map(types.map(row=>[typeKey(row),row]));
      const result=[{id:'all',label:'',options:[{value:'*',label:'All'}]}];
      for(const space of mergedSpaces(saved)){
        const rows=types.filter(row=>row.spaceId===space.id).map(row=>({...row}));
        for(const item of saved.types.items){
          const key=typeKey(item);
          if(item.spaceId===space.id&&!rows.some(row=>typeKey(row)===key))rows.push({...item});
        }
        if(saved.types.all&&!saved.spaces.all&&saved.spaces.items.some(row=>row.id===space.id)&&!rows.some(row=>row.id==='*')) {
          rows.unshift({spaceId:space.id,id:'*',name:'All types',space:space.name});
        }
        const options=rows.map(row=>{
          const discovered=known.get(typeKey(row));
          const icon=discovered?.icon||row.icon;
          return {value:typeKey(row),row:{...row,icon},label:row.id==='*'?'All types':row.name,icon,keywords:[space.name,row.space]};
        }).sort((a,b)=>a.label.localeCompare(b.label)||a.value.localeCompare(b.value));
        result.push({id:space.id,label:space.name,options});
      }
      return result;
    }
    function selectedValues(saved){
      if(saved.types.all&&saved.spaces.all)return['*'];
      if(saved.types.all)return saved.spaces.items.map(space=>wildcardKey(space.id));
      return saved.types.items.map(typeKey);
    }
    function optionsRows(){
      const rows=new Map();
      for(const section of snapshot.sections)for(const option of section.options)if(option.row)rows.set(option.value,option.row);
      return rows;
    }
    function change(values){
      if(!api)return;
      if(filters().types.all&&values.includes('*')&&values.length>1)values=values.filter(value=>value!=='*');
      const next=normalize(api.read()),all=values.includes('*');
      if(all){
        next.objectMentions.types={all:true,items:[]};
        next.objectMentions.spaces={all:true,items:[]};
      }else{
        const rows=optionsRows();
        const selectedTypes=values.filter(value=>value!=='*').map(value=>rows.get(value)).filter(Boolean)
          .map(row=>({spaceId:row.spaceId,id:row.id,name:row.name,space:row.space}));
        const spaceRows=new Map();
        for(const row of selectedTypes)if(!spaceRows.has(row.spaceId))spaceRows.set(row.spaceId,{id:row.spaceId,name:row.space});
        next.objectMentions.types={all:false,items:selectedTypes};
        next.objectMentions.spaces={all:false,items:[...spaceRows.values()]};
      }
      api.onChange(next);refresh();
    }
    function setIncompleteTasks(checked){if(!api)return;const next=normalize(api.read());next.objectMentions.filterIncompleteTasks=checked===true;api.onChange(next);refresh();}
    function refresh(){const selected=filters();publish({sections:buildSections(selected),selections:selected,filterIncompleteTasks:selected.filterIncompleteTasks});}
    async function discoverTypes(forceRefresh=false){
      const current=++epoch,ids=spaces.filter(space=>readable(space.id)).map(space=>space.id);
      if(!ids.length){status='';publish({status});refresh();return;}
      status='Loading object types…';publish({status});
      try{const result=await api.invoke('discover_anyblob_types',{spaceIds:ids,forceRefresh});if(current!==epoch)return;if(!Array.isArray(result?.types)||!Array.isArray(result?.errors))throw Error('Type discovery returned an invalid response.');const failed=new Set(result.errors.map(error=>error.spaceId));types=types.filter(type=>!ids.includes(type.spaceId)||failed.has(type.spaceId));const merged=new Map(types.map(type=>[typeKey(type),type]));for(const type of result.types)merged.set(typeKey(type),type);types=[...merged.values()];status=result.errors.map(error=>error.error).join(' ');}
      catch(error){if(current===epoch)status=String(error.message||error);}
      finally{if(current===epoch){publish({types,status});refresh();}}
    }
    async function discover(forceRefresh=false){
      if(pending||!api)return;pending=true;status='Loading spaces…';publish({status,pending});
      try{const result=await UI.SpaceCatalog.load(api.invoke,forceRefresh);if(!Array.isArray(result))throw Error('Space discovery returned an invalid response.');spaces=result;loaded=true;status='';publish({spaces,loaded,status});refresh();await discoverTypes(forceRefresh);}
      catch(error){status=String(error.message||error);publish({status});}
      finally{pending=false;publish({pending});refresh();}
    }
    return {subscribe,getSnapshot,init(options,normalizer){api=options;normalize=normalizer;UI.AppReload?.register(()=>discover(true));refresh();},selectedValues,change,setIncomplete:setIncompleteTasks,refresh,discover(){return discover(false);},show(){refresh();if(!loaded&&!pending)void discover();},discoverTypes,destroy(){epoch++;listeners.clear();}};
  }
  const controller=createController();
  UI.ObjectMentions={...controller,createController};
})(window);
