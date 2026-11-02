(function (root) {
  'use strict';
  const UI = root.BlobUI, C = UI.Components, doc = root.document;
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
  const make = UI.CatalogBrowser.node;
  let api, page, tabs, list, message, empty, browser, nav, loaded = false, busy = false, locked = true, records = [], pickers = [];
  function permissionRow({view, permission, disabled, onChange, selection, metadata, dialog, scrollContainer}) {
    const owned = [];
    const item = make('section','formula-view codex-space');
    const top = make('div','codex-space-top'), bottom = make('div','codex-space-bottom');
    if (selection) top.append(selection);
    const description = view.name + (selection ? '' : '\n' + view.id);
    const name = make('span','formula-view-name',view.name); name.title = description; top.append(name);
    const change = (key,value) => { if (!disabled) { permission[key]=value; onChange({...permission}); } };
      function picker(parent,key,options,label) {
        const shell = C.PropertyPickerShell({label,showValue:true,accessibleLabel:label+' for '+description,menuLabel:label});
        shell.trigger.disabled = disabled;
        parent.append(shell.element);
        if (dialog) dialog.append(shell.menu);
        const controller = C.Picker({root:shell.element,menu:shell.menu,options,value:permission[key],onChange:value=>change(key,value)});
        const place = () => {
          const rect = shell.trigger.getBoundingClientRect();
          const width = Math.min(300, root.innerWidth - 24);
          const below = root.innerHeight - rect.bottom - 12;
          const above = rect.top - 12;
          const openAbove = below < 220 && above > below;
          if (dialog) {
            const bounds=dialog.getBoundingClientRect();
            shell.menu.style.width=Math.min(width,bounds.width-24)+'px';
            shell.menu.style.left=Math.max(12,Math.min(rect.right-bounds.left-width,bounds.width-width-12))+'px';
            shell.menu.style.top=rect.bottom-bounds.top+6+'px';
            shell.menu.style.maxHeight=Math.max(80,root.innerHeight-rect.bottom-24)+'px';
            return;
          }
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
        scrollContainer?.addEventListener('scroll',dismiss);
        root.addEventListener('resize',dismiss);
        owned.push({destroy() { controller.destroy(); if(dialog)shell.menu.remove();scrollContainer?.removeEventListener('scroll',dismiss); for (const event of ['click','keydown']) shell.trigger.removeEventListener(event,place,true); doc.querySelector('.page-content').removeEventListener('scroll',dismiss); root.removeEventListener('resize',dismiss); }});
      }
    if (metadata) top.append(make('p','input-hint project-source-metadata',metadata));
    const toggle = C.ToggleSetting({label:'Upload/delete files',inputAttributes:{'aria-label':'Upload/delete files for '+description},onChange:()=>change('filesEnabled',toggle.input.checked)});
    toggle.input.checked = permission.filesEnabled; toggle.input.disabled = disabled; bottom.append(toggle.element);
    picker(bottom,'access',access,'Access');
    picker(bottom,'approval',approvals,'Approval'); item.append(top);
    item.append(bottom);
    return {item, destroy() { owned.forEach(p=>p.destroy()); C.destroyTree(item); }};
  }
  function render() {
    if (!list) return;
    doc.getElementById('codex-panel-0')?.refreshMentions?.();
    pickers.forEach(p => p.destroy()); pickers = []; C.destroyTree(list); list.replaceChildren();
    const preferences = normalize(api.read());
    records = records.map(({view}) => {
      const permission = preferences.spaces[view.id] || {access:'none',filesEnabled:false,approval:'approveForMe'};
      const row = permissionRow({view,permission,disabled:locked,onChange(value) {
        const next = normalize(api.read()); next.spaces[view.id] = value; api.onChange(next);
      }});
      pickers.push(row); list.append(row.item); return {item:row.item,view};
    });
    browser?.refresh();
  }
  async function reload(forceRefresh = false) {
    if (busy) return; busy = true; browser.setLocked(locked,true);
    message.textContent = 'Loading spaces…';
    try {
      const spaces = await api.invoke('discover_codex_spaces',{forceRefresh});
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
    const installRow=C.SettingsRow({label:'Install Marketplace',content:[install.element],attributes:{class:'settings-row kanban-setting-row'}}).element;
    const subtitle=make('span','anyblob-version');subtitle.hidden=true;
    installRow.querySelector('.kanban-setting-copy').append(subtitle);
    panel.append(C.Heading({title:'General'}).element,C.Group({content:[toggle.element,installRow]}).element,status);
    const mentions=UI.ObjectMentions.mount(panel,api,normalize);
    panel.refreshMentions=()=>mentions.refresh();
    let changing=false,installing=false,integrationEpoch=0,installationEpoch=0,installation=null;
    function showInstallation(value){installation=value;install.element.textContent=value?.updateAvailable?'Update':value?.installed?'Installed':'Install';install.element.title=value?.bundledVersion?'Anyblob '+value.bundledVersion:'';
      subtitle.replaceChildren();
      if(value?.installedVersion)subtitle.append(make('span','', 'v'+value.installedVersion));
      if(value?.updateAvailable&&value.bundledVersion){
        if(value.installedVersion)subtitle.append(doc.createTextNode(' → '));
        subtitle.append(make('span','anyblob-version-available','v'+value.bundledVersion));
      }
      subtitle.hidden=!subtitle.textContent;
    }
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
    panel.refreshIntegration=()=>{void refresh();void refreshInstallation();mentions.show();};
    void refresh();void refreshInstallation();

  }

  function init(options) {
    api = options;
    const navigation = doc.getElementById('sidebar-nav'); navigation.append(make('span','nav-group-label','Integrations'));
    nav = C.NavigationItem({label:'Anyblob',icon: (() => { const span=make('span','nav-icon'); span.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round">'+Array.from({length:6},(_,i)=>'<path transform="rotate('+i*60+' 12 12)" d="M12 3a4.5 4.5 0 0 1 7.8 4.5l-4.5 2.6v5.2"/>').join('')+'</svg>'; return span; })(),attributes:{id:'codex-nav',class:'nav-item','data-settings-page':'codex'},onClick:()=>api.navigate('codex')}).element; navigation.append(nav);
    page = make('div','settings-page');page.id='codex-page';page.hidden=true;doc.getElementById('advanced-page').after(page);
    const bar=make('div','advanced-tabs');bar.setAttribute('role','tablist');bar.setAttribute('aria-label','Anyblob');
    const panels=['Settings','Permissions','Projects'].map((label,index)=>{const button=make('button','',label);button.type='button';button.id='codex-tab-'+index;button.setAttribute('role','tab');const panel=make('section','');panel.id='codex-panel-'+index;panel.setAttribute('role','tabpanel');panel.setAttribute('aria-labelledby',button.id);button.setAttribute('aria-controls',panel.id);bar.append(button);return panel;});
    UI.AppReload?.register(() => reload(true));
    page.append(bar,...panels);tabs=C.Tabs({container:bar,buttons:[...bar.children],panels,onSelect:index=>{if(index===1&&!loaded&&!busy)void reload();if(index===2)UI.ProjectPermissions.open();}});
    mountIntegration(panels[0]);
    UI.ProjectPermissions.init({...api, panel:panels[2], permissionRow, normalize});
    const toolbar=UI.CatalogBrowser.toolbar('codex','Search spaces');toolbar.querySelector('#codex-filter-control').hidden=true;
    list=make('div','');message=make('p','input-hint');message.setAttribute('role','status');empty=make('p','input-hint');empty.hidden=true;panels[1].append(toolbar,message,list,empty);
    browser=UI.CatalogBrowser.attach({prefix:'codex',toolbar,records:()=>records,empty,onReload:()=>reload(true)});
  }
  UI.CodexPage={normalize,init,render(){render();UI.ProjectPermissions.render();},show(value){if(!page)return;page.hidden=!value;nav.classList.toggle('active',value);nav.setAttribute('aria-current',value?'page':'false');if(value){doc.getElementById('codex-panel-0').refreshIntegration();tabs.refresh();if(!doc.getElementById('codex-panel-2').hidden)UI.ProjectPermissions.open();if(!doc.getElementById('codex-panel-1').hidden&&!loaded&&!busy)void reload();}},select(index){tabs.select(index);if(index===2)UI.ProjectPermissions.open();if(index===1&&!loaded&&!busy)void reload();},setLocked(value){if(locked===value)return;locked=value;render();UI.ProjectPermissions.setLocked(value);browser?.setLocked(value,busy);}};
})(window);
