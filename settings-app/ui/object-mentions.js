(function (root) {
  'use strict';
  const UI = root.BlobUI, C = UI.Components, make = UI.CatalogBrowser.node;
  UI.ObjectMentions = { mount(panel, api, normalize) {
    let spaces = [], types = [], epoch = 0, pending = false, loaded = false;
    const status = make('p','input-hint'); status.setAttribute('role','status'); status.id='object-mentions-status';
    const note = make('p','input-hint','Filters control suggestions. Reading an object still requires permission, including Always Ask. Save to apply changes.');
    const group = C.Group({attributes:{id:'object-mentions-group'}});
    const reload = C.Button({label:'Reload options',attributes:{class:'banner-image-button'},onClick:()=>void discover()});
    const section=C.Section({attributes:{id:'object-mentions-section'},content:[C.Heading({title:'Object Mentions'}).element,group.element,note,status,reload.element]});
    panel.append(section.element);
    const pickers = {};
    const key = (kind,row) => kind==='spaces' ? row.id : JSON.stringify([row.spaceId,row.id]);
    const filters = () => normalize(api.read()).objectMentions;
    function options(kind) {
      const saved = filters(), selected = saved[kind], entries = new Map();
      const active = sid => saved.spaces.all || saved.spaces.items.some(s=>s.id===sid);
      const available = kind==='spaces' ? spaces : types.filter(t=>active(t.spaceId)&&spaces.some(s=>s.id===t.spaceId));
      for (const row of available) entries.set(key(kind,row),{row,available:true});
      for (const row of selected.items) if (!entries.has(key(kind,row))) entries.set(key(kind,row),{row,available:false});
      return [{value:'*',label:'All'},...[...entries].map(([value,{row,available}])=>({value,row,
        label:kind==='spaces' ? row.name : row.name+' · '+row.space,
        description:kind==='types'&&!active(row.spaceId) ? 'Inactive outside filtered spaces' : available ? '' : 'Unavailable saved selection'
      })).sort((a,b)=>a.label.localeCompare(b.label)||a.value.localeCompare(b.value))];
    }
    function change(kind,values) {
      const next = normalize(api.read()), before = next.objectMentions[kind];
      const all = values.includes('*') && !before.all;
      const selected = values.filter(v=>v!=='*');
      next.objectMentions[kind] = {all,items:all ? [] : options(kind).filter(o=>selected.includes(o.value)).map(o=>o.row)};
      api.onChange(next); refresh();
      if (kind==='spaces') void discoverTypes();
    }
    for (const [kind,label] of [['spaces','Filtered spaces'],['types','Filtered object types']]) {
      const shell = C.PropertyPickerShell({label:'All',showValue:true,accessibleLabel:label,menuLabel:label});
      const row = C.SettingsRow({label,content:[shell.element],attributes:{class:'settings-row kanban-setting-row object-mention-row'}});
      group.element.append(row.element);
      shell.menu.classList.add('app-scrollbar');
      const scrollbar=UI.Scrollbar.attach(shell.menu);
      const controller = C.Picker({root:shell.element,menu:shell.menu,multiple:true,closeOnSelect:false,onChange:values=>change(kind,values)});
      const place = () => {
        const rect=shell.trigger.getBoundingClientRect(), width=Math.min(390,root.innerWidth-32);
        shell.menu.style.width=width+'px';shell.menu.style.left=Math.max(16,Math.min(rect.right-width,root.innerWidth-width-16))+'px';
        const below=root.innerHeight-rect.bottom-16, above=rect.top-16, up=below<180&&above>below;
        shell.menu.style.top=up?'auto':rect.bottom+6+'px';shell.menu.style.bottom=up?root.innerHeight-rect.top+6+'px':'auto';
        shell.menu.style.maxHeight=Math.max(80,Math.min(360,up?above-6:below-6))+'px';
      };
      shell.trigger.addEventListener('click',place,true);shell.trigger.addEventListener('keydown',place,true);
      root.addEventListener('resize',()=>controller.close());
      root.document.querySelector('.page-content').addEventListener('scroll',()=>controller.close());
      pickers[kind]={shell,controller,scrollbar};
    }
    function refresh() {
      for (const kind of ['spaces','types']) {
        const selection=filters()[kind], picker=pickers[kind];
        const available=options(kind), signature=JSON.stringify(available);
        if(picker.signature!==signature){picker.controller.setOptions(available);picker.signature=signature;}
        picker.controller.setValue(selection.all ? ['*'] : selection.items.map(row=>key(kind,row)));
        picker.shell.trigger.querySelector('[data-pick-value]').textContent=selection.all?'All':selection.items.length?selection.items.length+' selected':'None';
        // These are suggestion filters, independent of the permission lock and discovery state.
        picker.shell.trigger.disabled=false;
        picker.scrollbar.update();
      }
    }
    async function discoverTypes() {
      const current=++epoch, saved=filters();
      const ids=spaces.filter(s=>saved.spaces.all||saved.spaces.items.some(v=>v.id===s.id)).map(s=>s.id);
      if (!ids.length) { status.textContent='';refresh(); return; }
      status.textContent='Loading object types…';
      try {
        const result=await api.invoke('discover_anyblob_types',{spaceIds:ids});
        if(current!==epoch)return;
        if(!Array.isArray(result?.types)||!Array.isArray(result?.errors))throw Error('Type discovery returned an invalid response.');
        const failed=new Set(result.errors.map(e=>e.spaceId));
        types=types.filter(t=>!ids.includes(t.spaceId)||failed.has(t.spaceId));
        const merged=new Map(types.map(t=>[key('types',t),t]));
        for(const type of result.types)merged.set(key('types',type),type);
        types=[...merged.values()];status.textContent=result.errors.map(e=>e.error).join(' ');
      } catch(error) {if(current===epoch)status.textContent=String(error.message||error);}
      finally {if(current===epoch)refresh();}
    }
    async function discover() {
      if(pending)return;pending=true;status.textContent='Loading spaces…';
      try {
        const result=await api.invoke('discover_codex_spaces');
        if(!Array.isArray(result))throw Error('Space discovery returned an invalid response.');
        spaces=result;loaded=true;status.textContent='';refresh();await discoverTypes();
      } catch(error) {status.textContent=String(error.message||error);}
      finally {pending=false;refresh();}
    }
    refresh();
    return {refresh,show(){refresh();if(!loaded&&!pending)void discover();}};
  }};
})(window);
