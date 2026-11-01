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

  function mountIntegration(panel) {
    const status=make('p','input-hint');status.setAttribute('role','status');
    const toggle=C.ToggleSetting({label:'Enable Anyblob',onChange:()=>configure(toggle.input.checked)});
    const install=C.Button({label:'Install',attributes:{class:'banner-image-button update-check-button'},onClick:()=>void installMarketplace()});
    panel.append(C.Heading({title:'General'}).element,C.Group({content:[toggle.element,C.SettingsRow({label:'Install Marketplace',content:[install.element],attributes:{class:'settings-row kanban-setting-row'}}).element]}).element,status);
    let changing=false,installing=false,integrationEpoch=0,installationEpoch=0,installation=null;
    function showInstallation(value){installation=value;install.element.textContent=value?.updateAvailable?'Update':value?.installed?'Installed':'Install';install.element.title=value?.bundledVersion?'Anyblob '+value.bundledVersion:'';}
    async function configure(enabled) {
      if(changing)return;changing=true;integrationEpoch++;toggle.input.disabled=true;status.textContent='';
      try {const value=await api.invoke('configure_anyblob',{enabled});toggle.input.checked=value.enabled;}
      catch(error){status.textContent=String(error.message||error);await refresh();}
      finally {changing=false;toggle.input.disabled=false;}
    }
    async function refresh(){const epoch=++integrationEpoch;try{const value=await api.invoke('anyblob_status');if(epoch===integrationEpoch)toggle.input.checked=value.enabled;}catch(error){if(epoch===integrationEpoch)status.textContent=String(error.message||error);}}
    async function refreshInstallation(){if(installing)return;const epoch=++installationEpoch;try{const value=await api.invoke('anyblob_installation_status');if(epoch===installationEpoch)showInstallation(value);}catch(error){if(epoch===installationEpoch){showInstallation(null);status.textContent=String(error.message||error);}}}
    async function installMarketplace(){
      if(installing)return;const updating=!!installation?.updateAvailable;installing=true;installationEpoch++;install.element.disabled=true;install.element.textContent=updating?'Updating…':'Installing…';status.textContent='';
      try {
        await api.invoke('install_anyblob_marketplace');showInstallation(await api.invoke('anyblob_installation_status'));
        const title=make('h2','',updating?'Anyblob marketplace updated':'Anyblob marketplace installed');
        const text=make('p','',updating?'The bundled Anyblob release is registered. Open Codex’s Plugins Directory and refresh Anyblob. If it still uses the previous release, uninstall and reinstall Anyblob there, then reopen Codex. BlobUI does not replace Codex’s running plugin copy.':'Open the Plugins Directory, select your personal marketplace, then choose Anyblob to install and enable it. You may need to reopen Codex. Space access still requires saved BlobUI permissions.');
        let modal;
        const done=C.Button({label:'Done',attributes:{class:'confirm-cancel'},onClick:()=>modal.close()});
        const actions=make('div','confirm-actions');actions.append(done.element);
        const shell=C.DialogShell({dialogAttributes:{'aria-label':title.textContent},content:[title,text,actions]});doc.body.append(shell.element);
        modal=C.Modal({backdrop:shell.element,dialog:shell.dialog,initialFocus:done.element,inertTargets:[...doc.body.children].filter(node=>node!==shell.element&&node.tagName!=='SCRIPT'),onClose:()=>root.setTimeout(()=>{modal.destroy();shell.destroy();shell.element.remove();},180)});modal.open();
      }catch(error){showInstallation(installation);status.textContent=String(error.message||error);}
      finally{installing=false;install.element.disabled=false;}
    }
    panel.refreshIntegration=()=>{void refresh();void refreshInstallation();};
    panel.refreshIntegration();
    const eventApi=root.__TAURI__?.event;
    if(!eventApi)return;
    let current=null,closing=null;
    function cleanup(previous){if(previous.timer)root.clearTimeout(previous.timer);previous.modal.destroy();previous.shell.destroy();previous.shell.element.remove();if(closing===previous)closing=null;}
    function close(){if(!current)return;const previous=current;current=null;previous.modal.close();closing=previous;previous.timer=root.setTimeout(()=>cleanup(previous),180);}
    eventApi.listen('anyblob-approval',({payload})=>{
      close();if(closing)cleanup(closing);
      const request=payload.request;
      const title=make('h2','','Allow Anyblob read?');
      const text=make('p','');text.style.whiteSpace='pre-wrap';text.style.overflowWrap='anywhere';text.style.maxHeight='40vh';text.style.overflow='auto';
      text.textContent=(request.operation==='search_objects'?'Search objects':'Read object')+'\nSpace ID: '+request.space_id+'\n'+(request.operation==='search_objects'?'Query: '+request.query+'\nLimit: '+request.limit+' | Offset: '+request.offset:'Object ID: '+request.object_id);
      const decide=async allow=>{if(current?.id!==payload.id)return;try{await api.invoke('approve_anyblob',{id:payload.id,allow});}catch{}finally{if(current?.id===payload.id)close();}};
      const deny=C.Button({label:'Deny',attributes:{class:'confirm-cancel'},onClick:()=>void decide(false)});
      const allow=C.Button({label:'Allow once',attributes:{class:'confirm-save'},onClick:()=>void decide(true)});
      const actions=make('div','confirm-actions');actions.append(deny.element,allow.element);
      const shell=C.DialogShell({dialogAttributes:{'aria-label':'Anyblob read approval'},content:[title,text,actions]});doc.body.append(shell.element);
      const modal=C.Modal({backdrop:shell.element,dialog:shell.dialog,initialFocus:deny.element,inertTargets:[...doc.body.children].filter(node=>node!==shell.element&&node.tagName!=='SCRIPT'),onClose:()=>{if(current?.id===payload.id)void decide(false);}});
      current={id:payload.id,modal,shell};modal.open();
    }).catch(()=>{});
    eventApi.listen('anyblob-approval-closed',({payload})=>{if(current?.id===payload.id)close();}).catch(()=>{});
    root.addEventListener('pagehide',()=>{if(current)void api.invoke('approve_anyblob',{id:current.id,allow:false}).catch(()=>{});});
  }

  function init(options) {
    api = options;
    const navigation = doc.getElementById('sidebar-nav'); navigation.append(make('span','nav-group-label','Integrations'));
    nav = C.NavigationItem({label:'Anyblob',icon: (() => { const span=make('span','nav-icon'); span.innerHTML='<svg viewBox="0 0 20 20" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5"><path d="m7 5-5 5 5 5m6-10 5 5-5 5m-2-12-2 14"/></svg>'; return span; })(),attributes:{id:'codex-nav',class:'nav-item','data-settings-page':'codex'},onClick:()=>api.navigate('codex')}).element; navigation.append(nav);
    page = make('div','settings-page');page.id='codex-page';page.hidden=true;doc.getElementById('advanced-page').after(page);
    const bar=make('div','advanced-tabs');bar.setAttribute('role','tablist');bar.setAttribute('aria-label','Anyblob');
    const panels=['Settings','Permissions'].map((label,index)=>{const button=make('button','',label);button.type='button';button.id='codex-tab-'+index;button.setAttribute('role','tab');const panel=make('section','');panel.id='codex-panel-'+index;panel.setAttribute('role','tabpanel');panel.setAttribute('aria-labelledby',button.id);button.setAttribute('aria-controls',panel.id);bar.append(button);return panel;});
    page.append(bar,...panels);tabs=C.Tabs({container:bar,buttons:[...bar.children],panels,onSelect:index=>{if(index===1&&!loaded&&!busy)void reload();}});
    mountIntegration(panels[0]);
    const toolbar=UI.CatalogBrowser.toolbar('codex','Search spaces');toolbar.querySelector('#codex-filter-control').hidden=true;
    list=make('div','');message=make('p','input-hint');message.setAttribute('role','status');empty=make('p','input-hint');empty.hidden=true;panels[1].append(toolbar,message,list,empty);
    browser=UI.CatalogBrowser.attach({prefix:'codex',toolbar,records:()=>records,empty,onReload:reload});
  }
  UI.CodexPage={normalize,init,render,show(value){if(!page)return;page.hidden=!value;nav.classList.toggle('active',value);nav.setAttribute('aria-current',value?'page':'false');if(value){doc.getElementById('codex-panel-0').refreshIntegration();tabs.refresh();if(!doc.getElementById('codex-panel-1').hidden&&!loaded&&!busy)void reload();}},select(index){tabs.select(index);if(index===1&&!loaded&&!busy)void reload();},setLocked(value){if(locked===value)return;locked=value;render();browser?.setLocked(value,busy);}};
})(window);
