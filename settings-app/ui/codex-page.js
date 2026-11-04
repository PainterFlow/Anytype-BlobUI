(function (root) {
  'use strict';
  const UI = root.BlobUI;
  const access = [{value:'none',label:'No Access'}, {value:'read',label:'Read Only'}, {value:'readWrite',label:'Read-Write'}, {value:'createDelete',label:'Create-Delete'}];
  const approvals = [{value:'alwaysAsk',label:'Always Ask'}, {value:'approveForMe',label:'Approve for me'}, {value:'fullAccess',label:'Never Ask'}];
  function normalize(value) {
    const spaces = Object.create(null);
    const entries = value?.spaces && typeof value.spaces === 'object' && !Array.isArray(value.spaces) ? value.spaces : {};
    for (const [id, entry] of Object.entries(entries)) spaces[id] = {
      access: access.some(o => o.value === entry?.access) ? entry.access : 'none',
      filesEnabled: entry?.filesEnabled === true,
      approval: approvals.some(o => o.value === entry?.approval) ? entry.approval : 'approveForMe'
    };
    const filters = value?.objectMentions;
    const selection = (value, keys) => value?.all === true ? {all:true,items:[]} : {
      all:false, items:Array.isArray(value?.items) ? value.items.filter(row => row && keys.every(key => typeof row[key] === 'string')).map(row => Object.fromEntries(keys.map(key => [key,row[key]]))) : []
    };
    const objectMentions = filters === undefined ? {spaces:{all:true,items:[]},types:{all:true,items:[]}} : {
      spaces:selection(filters?.spaces,['id','name']),
      types:selection(filters?.types,['spaceId','id','name','space'])
    };
    objectMentions.filterIncompleteTasks=filters?.filterIncompleteTasks===true;
    return {spaces, projects: UI.ProjectPermissions.normalize(value?.projects), objectMentions};
  }
  let api, page, loaded = false, busy = false, locked = true, records = [], attached = false, apiSnapshot=null, selectedTab=0;
  const listeners=new Set();
  const tabListeners=new Set();
  const permissionListeners=new Set();
  let permissionSnapshot={spaces:[],busy:false,locked:true,message:''};
  function publishPermissions(patch={}) { permissionSnapshot={...permissionSnapshot,...patch};permissionListeners.forEach(listener=>listener()); }
  function render() {
    UI.ObjectMentions?.refresh?.();
    publishPermissions({spaces:records.slice(),locked,busy,message:permissionSnapshot.message});
  }
  async function reload(forceRefresh = false) {
    if (busy) return; busy = true;publishPermissions({busy:true,locked,message:'Loading spaces…'});
    try {
      const spaces = await api.invoke('discover_codex_spaces',{forceRefresh});
      if (!Array.isArray(spaces)) throw new Error('Space discovery returned an invalid response.');
      records = spaces.map(space=>({...space,target:{spaceId:space.id}})); loaded = true;
      permissionSnapshot.message=spaces.length?'':'No accessible spaces found.';render();
    } catch(error) {
      permissionSnapshot.message=String(error.message||error).replace(' to use Kanban formulas',' to list spaces');
      publishPermissions({message:permissionSnapshot.message});
    } finally { busy = false;publishPermissions({busy:false}); }
  }

  function initialize() {
    if (!api || !page || attached) return;
    attached = true;
    UI.ObjectMentions.init(api,normalize);
    UI.ProjectPermissions.init({...api, normalize});
    publishPermissions({spaces:records,locked,busy,message:permissionSnapshot.message});
  }
  function init(options) {
    api = options;
    apiSnapshot=api;listeners.forEach(listener=>listener());
    initialize();
    UI.AppReload?.register(() => reload(true));
  }
  UI.CodexPage={normalize,subscribe(listener){listeners.add(listener);return()=>listeners.delete(listener);},getSnapshot(){return apiSnapshot;},subscribeTabs(listener){tabListeners.add(listener);return()=>tabListeners.delete(listener);},getTabSnapshot(){return selectedTab;},subscribePermissions(listener){permissionListeners.add(listener);return()=>permissionListeners.delete(listener);},getPermissionsSnapshot(){return permissionSnapshot;},getSpaces(){return records.slice();},reloadSpaces(){return reload(true);},changePermission(id,permission){if(locked)return;const next=normalize(api.read());next.spaces[id]=permission;api.onChange(next);render();},attach({root:host}){page=host;initialize();},init,render(){render();UI.ProjectPermissions.render();},show(value){if(!page)return;if(value){root.dispatchEvent(new Event('blobui-codex-integration-refresh'));UI.ObjectMentions.show();if(selectedTab===2)UI.ProjectPermissions.open();if(selectedTab===1&&!loaded&&!busy)void reload();}},select(index){if(!Number.isInteger(index)||index<0||index>2)return;if(selectedTab!==index){selectedTab=index;tabListeners.forEach(listener=>listener());}if(index===2)UI.ProjectPermissions.open();if(index===1&&!loaded&&!busy)void reload();},setLocked(value){if(locked===value)return;locked=value;render();UI.ProjectPermissions.setLocked(value);},destroy(){UI.ProjectPermissions.destroy?.();UI.ObjectMentions.destroy?.();attached=false;page=null;}};
})(window);
