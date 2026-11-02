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
    panel.append(C.Heading({title:'General'}).element,C.Group({content:[toggle.element,C.SettingsRow({label:'Install Marketplace',content:[install.element],attributes:{class:'settings-row kanban-setting-row'}}).element]}).element,status);
    const mentions=UI.ObjectMentions.mount(panel,api,normalize);
    panel.refreshMentions=()=>mentions.refresh();
    let changing=false,installing=false,integrationEpoch=0,installationEpoch=0,installation=null;
    function showInstallation(value){installation=value;install.element.textContent=value?.updateAvailable?'Update':value?.installed?'Installed':'Install';install.element.title=value?.bundledVersion?'Anyblob '+value.bundledVersion:'';
      const version=value?.updateAvailable?value.bundledVersion:value?.installedVersion;
      const subtitle=nav.querySelector('.anyblob-version');
      subtitle.textContent=version?'v'+version+(value.updateAvailable?' available':''):'';
      subtitle.hidden=!version;
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
    nav = C.NavigationItem({label:'Anyblob',icon: (() => { const span=make('span','nav-icon'); span.innerHTML='<svg viewBox="0 0 24 24" aria-hidden="true" fill="currentColor"><path d="M22.2819 9.8211a5.9847 5.9847 0 0 0-.5157-4.9108 6.0462 6.0462 0 0 0-6.5098-2.9A6.0651 6.0651 0 0 0 4.9807 4.1818a5.9847 5.9847 0 0 0-3.9977 2.9 6.0462 6.0462 0 0 0 .7427 7.0966 5.98 5.98 0 0 0 .511 4.9107 6.051 6.051 0 0 0 6.5146 2.9001A5.9847 5.9847 0 0 0 13.2599 24a6.0557 6.0557 0 0 0 5.7718-4.2058 5.9894 5.9894 0 0 0 3.9977-2.9001 6.0557 6.0557 0 0 0-.7475-7.0729zm-9.022 12.6081a4.4755 4.4755 0 0 1-2.8764-1.0408l.1419-.0804 4.7783-2.7582a.7948.7948 0 0 0 .3927-.6813v-6.7369l2.02 1.1686a.071.071 0 0 1 .038.052v5.5826a4.504 4.504 0 0 1-4.4945 4.4944zm-9.6607-4.1254a4.4708 4.4708 0 0 1-.5346-3.0137l.142.0852 4.783 2.7582a.7712.7712 0 0 0 .7806 0l5.8428-3.3685v2.3324a.0804.0804 0 0 1-.0332.0615L9.74 19.9502a4.4992 4.4992 0 0 1-6.1408-1.6464zM2.3408 7.8956a4.485 4.485 0 0 1 2.3655-1.9728V11.6a.7664.7664 0 0 0 .3879.6765l5.8144 3.3543-2.0201 1.1685a.0757.0757 0 0 1-.071 0l-4.8303-2.7865A4.504 4.504 0 0 1 2.3408 7.872zm16.5963 3.8558L13.1038 8.364 15.1192 7.2a.0757.0757 0 0 1 .071 0l4.8303 2.7913a4.4944 4.4944 0 0 1-.6765 8.1042v-5.6772a.79.79 0 0 0-.407-.667zm2.0107-3.0231l-.142-.0852-4.7735-2.7818a.7759.7759 0 0 0-.7854 0L9.409 9.2297V6.8974a.0662.0662 0 0 1 .0284-.0615l4.8303-2.7866a4.4992 4.4992 0 0 1 6.6802 4.66zM8.3065 12.863l-2.02-1.1638a.0804.0804 0 0 1-.038-.0567V6.0742a4.4992 4.4992 0 0 1 7.3757-3.4537l-.142.0805L8.704 5.459a.7948.7948 0 0 0-.3927.6813zm1.0976-2.3654l2.602-1.4998 2.6069 1.4998v2.9994l-2.5974 1.4997-2.6067-1.4997Z"/></svg>'; return span; })(),attributes:{id:'codex-nav',class:'nav-item','data-settings-page':'codex'},onClick:()=>api.navigate('codex')}).element; const label=nav.querySelector('.documentation-item-label');
    label.append(make('span','anyblob-version'));
    nav.querySelector('.anyblob-version').hidden=true;
    navigation.append(nav);
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
