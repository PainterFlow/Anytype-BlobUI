(function (root) {
  'use strict';
  const UI = root.BlobUI, C = UI.Components, doc = root.document;
  const access = [{value:'none',label:'No Access'}, {value:'read',label:'Read Only'}, {value:'readWrite',label:'Read-Write'}, {value:'createDelete',label:'Create-Delete'}];
  const approvals = [{value:'alwaysAsk',label:'Always Ask'}, {value:'approveForMe',label:'Approve for me'}, {value:'fullAccess',label:'Full Access'}];
  function normalize(value) {
    const spaces = Object.create(null);
    const entries = value?.spaces && typeof value.spaces === 'object' && !Array.isArray(value.spaces) ? value.spaces : {};
    for (const [id, entry] of Object.entries(entries)) spaces[id] = {
      access: access.some(o => o.value === entry?.access) ? entry.access : 'none',
      filesEnabled: entry?.filesEnabled === true,
      approval: approvals.some(o => o.value === entry?.approval) ? entry.approval : 'approveForMe'
    };
    return {spaces};
  }
  const make = UI.CatalogBrowser.node;
  let api, page, tabs, list, message, empty, browser, nav, loaded = false, busy = false, locked = true, records = [], pickers = [];
  function render() {
    if (!list) return;
    pickers.forEach(p => p.destroy()); pickers = []; C.destroyTree(list); list.replaceChildren();
    const preferences = normalize(api.read());
    records = records.map(({view}) => {
      const item = make('section','formula-view codex-space');
      const top = make('div','codex-space-top'), bottom = make('div','codex-space-bottom');
      const name = make('span','formula-view-name',view.name); name.title = view.name + '\n' + view.id; top.append(name);
      const permission = preferences.spaces[view.id] || {access:'none',filesEnabled:false,approval:'approveForMe'};
      const change = (key,value) => { if (locked) return; const next = normalize(api.read()); next.spaces[view.id] = {...permission,[key]:value}; Object.assign(permission,next.spaces[view.id]); api.onChange(next); };
      function picker(parent,key,options,label) {
        const shell = C.PropertyPickerShell({label,showValue:true,accessibleLabel:label+' for '+view.name+' '+view.id,menuLabel:label});
        shell.trigger.disabled = locked;
        parent.append(shell.element);
        const controller = C.Picker({root:shell.element,options,value:permission[key],onChange:value=>change(key,value)});
        const place = () => {
          const rect = shell.trigger.getBoundingClientRect();
          const width = Math.min(300, root.innerWidth - 24);
          const below = root.innerHeight - rect.bottom - 12;
          const above = rect.top - 12;
          const openAbove = below < 220 && above > below;
          shell.menu.style.width = width + 'px';
          shell.menu.style.left = Math.max(12, Math.min(rect.right - width, root.innerWidth - width - 12)) + 'px';
          shell.menu.style.right = 'auto';
          shell.menu.style.top = openAbove ? 'auto' : rect.bottom + 6 + 'px';
          shell.menu.style.bottom = openAbove ? root.innerHeight - rect.top + 6 + 'px' : 'auto';
          shell.menu.style.maxHeight = Math.max(80, openAbove ? above - 6 : below - 6) + 'px';
        };
        for (const event of ['click','keydown']) shell.trigger.addEventListener(event, place, true);
        const dismiss = () => controller.close();
        doc.querySelector('.page-content').addEventListener('scroll',dismiss);
        root.addEventListener('resize',dismiss);
        pickers.push({destroy() { controller.destroy(); for (const event of ['click','keydown']) shell.trigger.removeEventListener(event,place,true); doc.querySelector('.page-content').removeEventListener('scroll',dismiss); root.removeEventListener('resize',dismiss); }});
      }
      picker(top,'access',access,'Access');
      const toggle = C.ToggleSetting({label:'Upload/delete files',inputAttributes:{'aria-label':'Upload/delete files for '+view.name+' '+view.id},onChange:()=>change('filesEnabled',toggle.input.checked)});
      toggle.input.checked = permission.filesEnabled; toggle.input.disabled = locked; bottom.append(toggle.element);
      picker(bottom,'approval',approvals,'Approval'); item.append(top,bottom); list.append(item); return {item,view};
    });
    browser?.refresh();
  }
  async function reload() {
    if (busy) return; busy = true; browser.setLocked(locked,true);
    message.textContent = 'Loading spaces…';
    try {
      const spaces = await api.invoke('discover_codex_spaces');
      if (!Array.isArray(spaces)) throw new Error('Space discovery returned an invalid response.');
      records = spaces.map(space=>({view:{...space,target:{spaceId:space.id}}})); loaded = true;
      message.replaceChildren(); if (!spaces.length) message.textContent = 'No accessible spaces found.'; render();
    } catch(error) {
      UI.CatalogBrowser.status(message, String(error.message || error).replace(' to use Kanban formulas',' to list spaces'));
      if (message.textContent.includes('API key')) message.append(C.Button({label:'Open Advanced Settings > API key',onClick:()=>{api.navigate('advanced');doc.getElementById('advanced-api-tab').click();},attributes:{class:'formula-button'}}).element);
    } finally { busy = false; browser.setLocked(locked); }
  }
  function init(options) {
    api = options;
    const navigation = doc.getElementById('sidebar-nav'); navigation.append(make('span','nav-group-label','Integrations'));
    nav = C.NavigationItem({label:'Codex',icon: (() => { const span=make('span','nav-icon'); span.innerHTML='<svg viewBox="0 0 20 20" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5"><path d="m7 5-5 5 5 5m6-10 5 5-5 5m-2-12-2 14"/></svg>'; return span; })(),attributes:{id:'codex-nav',class:'nav-item','data-settings-page':'codex'},onClick:()=>api.navigate('codex')}).element; navigation.append(nav);
    page = make('div','settings-page');page.id='codex-page';page.hidden=true;doc.getElementById('advanced-page').after(page);
    const bar=make('div','advanced-tabs');bar.setAttribute('role','tablist');bar.setAttribute('aria-label','Codex');
    const panels=['Settings','Permissions'].map((label,index)=>{const button=make('button','',label);button.type='button';button.id='codex-tab-'+index;button.setAttribute('role','tab');const panel=make('section','');panel.id='codex-panel-'+index;panel.setAttribute('role','tabpanel');panel.setAttribute('aria-labelledby',button.id);button.setAttribute('aria-controls',panel.id);bar.append(button);return panel;});
    page.append(bar,...panels);tabs=C.Tabs({container:bar,buttons:[...bar.children],panels,onSelect:index=>{if(index===1&&!loaded&&!busy)void reload();}});
    const toolbar=UI.CatalogBrowser.toolbar('codex','Search spaces');toolbar.querySelector('#codex-filter-control').hidden=true;
    list=make('div','');message=make('p','input-hint');message.setAttribute('role','status');empty=make('p','input-hint');empty.hidden=true;panels[1].append(toolbar,message,list,empty);
    browser=UI.CatalogBrowser.attach({prefix:'codex',toolbar,records:()=>records,empty,onReload:reload});
  }
  UI.CodexPage={normalize,init,render,show(value){if(!page)return;page.hidden=!value;nav.classList.toggle('active',value);nav.setAttribute('aria-current',value?'page':'false');if(value){tabs.refresh();if(!doc.getElementById('codex-panel-1').hidden&&!loaded&&!busy)void reload();}},select(index){tabs.select(index);if(index===1&&!loaded&&!busy)void reload();},setLocked(value){if(locked===value)return;locked=value;render();browser?.setLocked(value,busy);}};
})(window);
