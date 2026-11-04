const test=require('node:test');
const assert=require('node:assert/strict');
const path=require('node:path');
const fs=require('node:fs');
const { loadSettings, openFormulas } = require('./frontend-fixture.cjs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const workspace=path.resolve(__dirname,'../..');

test('startup and Ctrl+R refresh hidden catalogs without changing drafts or publishing',async()=>{
  const temp=path.join(workspace,'.test-run/metadata-refresh/temp');fs.mkdirSync(temp,{recursive:true});process.env.TEMP=process.env.TMP=temp;
  const browser=await chromium.launch({executablePath:process.env.CHROME_PATH||'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true});
  const page=await browser.newPage({colorScheme:'dark',viewport:{width:1050,height:1000}});
  const errors=[];page.on('pageerror',error=>errors.push(error.message));
  await page.addInitScript(()=>{
    window.calls=[];window.network=0;window.revision=0;window.hold=false;
    const cache=new Map([[JSON.stringify(["discover_kanban_views",undefined]),{views:[],info:[]}]]);
    window.__TAURI__={core:{invoke:async(name,args={})=>{
      window.calls.push({name,args});
      if(name==='get_saved_state_snapshot') {
        const invoke=window.__TAURI__.core.invoke;
        return {app:await invoke('get_app_state'),updater:await invoke('get_updater_state'),formulas:await invoke('load_kanban_formulas'),calendar:await invoke('load_calendar_rules')};
      }
      if(name==='get_app_state')return {ready:true,theme:'dark',onboardingComplete:true,hasSavedApiKey:true,settings:{codexIntegration:{spaces:{s:{access:'read',filesEnabled:false,approval:'alwaysAsk'}},objectMentions:{spaces:{all:false,items:[{id:'s',name:'Work'}]},types:{all:false,items:[{spaceId:'s',id:'t',name:'Task',space:'Work'}]},filterIncompleteTasks:false}}}};
      if(name==='load_kanban_formulas')return {configuration:{version:2,display:'balanced',definitions:[]}};
      if(name==='load_calendar_rules')return {version:1,types:[]};
      if(name==='get_calendar_api_availability')return 'available';
      if(name==='get_keyboard_shortcuts'||name==='save_keyboard_shortcuts')return [{action:'openSettings',code:'KeyS',ctrl:true,alt:false,shift:false,meta:false}];
      if(name==='anyblob_status')return {enabled:false};
      if(name.startsWith('discover_')&&name!=='discover_codex_projects'){
        const key=JSON.stringify([name,args.spaceIds]);
        if(!args.forceRefresh&&cache.has(key))return cache.get(key);
        window.network++;
        if(window.hold)await new Promise(resolve=>{(window.releases??=[]).push(resolve);});
        let result;
        if(name==='discover_kanban_views')result={views:[{target:{spaceId:'s',objectId:'o',blockId:'b',viewId:'v'},name:'Board',space:'Work',source:'Tasks',kind:'Kanban',properties:[{key:'n',name:window.revision?'Updated points':'Points',format:'number'}]}],info:[]};
        if(name==='discover_calendar_colors')result={types:[],views:[]};
        if(name==='discover_codex_spaces')result=[{id:'s',name:'Work'}];
        if(name==='discover_anyblob_types')result={types:window.revision?[]:[{spaceId:'s',id:'t',name:'Task',space:'Work'}],errors:[]};
        if(name==='discover_codex_sources')result=[];
        cache.set(key,result);return result;
      }
      return null;
    }}};
  });
  const refresh=()=>page.evaluate(()=>{document.dispatchEvent(new KeyboardEvent('keydown',{key:'r',ctrlKey:true,bubbles:true,cancelable:true}));return window.BlobUI.AppReload.run();});
  try{
    await loadSettings(page);
    await page.waitForFunction(()=>window.appReady && window.calls.some(c=>c.name==='discover_anyblob_types') && !document.getElementById('object-mentions-status').textContent.includes('Loading'));
    const startup=await page.evaluate(()=>window.calls.filter(c=>c.name.startsWith('discover_')));
    for(const name of ['discover_kanban_views','discover_calendar_colors','discover_codex_spaces','discover_codex_sources','discover_anyblob_types']){
      assert.equal(startup.filter(c=>c.name===name).length,1,name+' refreshes once at startup');
      assert.equal(startup.find(c=>c.name===name).args.forceRefresh,true,name+' bypasses persistent cache at startup');
    }
    assert(!startup.some(c=>c.name==='discover_codex_projects'));
    assert.equal(await page.evaluate(()=>window.calls.some(c=>/^(save_|evaluate_|configure_)/.test(c.name))),false);
    assert.equal(await page.getByRole('button',{name:'Reload options',exact:true}).count(),0);
    await page.evaluate(()=>{window.hold=true;window.calls=[];});
    await page.keyboard.press('Control+r');await page.keyboard.press('Control+r');
    await page.waitForFunction(()=>window.calls.some(c=>c.name==='discover_codex_sources'));
    await page.evaluate(()=>{window.refreshDone=window.BlobUI.AppReload.run();});
    await page.evaluate(()=>{window.hold=false;window.releases.forEach(resolve=>resolve());});
    await page.evaluate(()=>window.refreshDone);
    assert.equal(await page.locator('#log-entries').locator('.log-entry.info').filter({hasText:'Reloading app metadata. Refreshing Anytype lists and pickers.'}).count(),2);
    const calls=await page.evaluate(()=>window.calls);
    for(const name of ['discover_kanban_views','discover_calendar_colors','discover_codex_spaces','discover_codex_sources','discover_anyblob_types']){
      assert.equal(calls.filter(c=>c.name===name).length,1,name+' is coalesced across repeated shortcuts and consumers');
      assert.equal(calls.find(c=>c.name===name).args.forceRefresh,true);
    }
    assert(!calls.some(c=>c.name==='discover_codex_projects'),'Local project discovery stays separate');
    await openFormulas(page);await page.getByRole('button',{name:'Board',exact:false}).first().click();
    const dialog=page.locator('#formula-calculator-dialog');await dialog.waitFor({state:'visible'});
    await dialog.getByRole('button',{name:'Show LaTeX',exact:true}).click();
    const source=dialog.getByRole('textbox',{name:'LaTeX source'});await source.fill('7 + 3');
    await dialog.getByRole('button',{name:'Insert property',exact:true}).click();
    await dialog.getByRole('option',{name:'Points',exact:true}).waitFor();
    await page.evaluate(()=>{window.revision=1;window.calls=[];});await refresh();
    assert.equal(await source.inputValue(),'7 + 3');
    assert.equal(await dialog.isVisible(),true);
    await dialog.getByRole('option',{name:'Updated points',exact:true}).waitFor();
    assert.equal(await page.evaluate(()=>window.calls.some(c=>/^(save_|evaluate_|configure_)/.test(c.name))),false);
    // The calculator stays open, so close through its normal draft action before navigating.
    await dialog.getByRole('button',{name:'Insert property',exact:true}).click();await dialog.getByRole('button',{name:'Save calculation',exact:true}).click();
    const requests=await page.evaluate(()=>window.network);
    await page.locator('#codex-nav').click();await page.locator('#codex-tab-1').click();await page.locator('#codex-tab-0').click();
    assert.equal(await page.evaluate(()=>window.network),requests,'Navigation uses cached catalogs');
    const spaces=page.getByRole('button',{name:'Filtered spaces',exact:true});
    const types=page.getByRole('button',{name:'Filtered object types',exact:true});
    assert.equal(await spaces.innerText(),'1 selected');assert.equal(await types.innerText(),'1 selected');
    await types.click();
    assert.match(await page.getByRole('option',{name:'Task · Work',exact:true}).innerText(),/Unavailable saved selection/);
    await types.click();
    await page.evaluate(()=>window.BlobUI.AppReload.setLocked(true));
    await page.keyboard.press('Control+r');await page.keyboard.press('Control+r');
    assert.equal(await page.evaluate(()=>window.network),requests);
    await page.evaluate(()=>{window.BlobUI.AppReload.setLocked(false);return window.BlobUI.AppReload.run();});
    assert.equal(await page.evaluate(()=>window.network),requests+5);
    await page.locator('#codex-tab-1').click();
    assert.match(await page.getByRole('button',{name:/Access for Work/}).innerText(),/Read/);
    assert.match(await page.getByRole('button',{name:/Approval for Work/}).innerText(),/Always Ask/);
    assert.equal(await page.getByRole('switch',{name:/Upload.delete files for Work/}).isChecked(),false);
    await openFormulas(page);await page.evaluate(()=>{window.calls=[];});
    await page.locator('#formula-reload').click();await page.waitForFunction(()=>window.calls.some(c=>c.name==='discover_kanban_views'));
    assert.deepEqual(await page.evaluate(()=>window.calls.filter(c=>c.name.startsWith('discover_')).map(c=>c.name)),['discover_kanban_views']);
    assert.equal(await page.evaluate(()=>window.calls.find(c=>c.name==='discover_kanban_views').args.forceRefresh),true);
    await page.evaluate(()=>document.getElementById('help-keybinds').click());
    await page.locator('#keybinds-dialog').waitFor({state:'visible'});
    const localShortcut=page.locator('#keybind-button-refreshMetadata');
    assert.equal(await localShortcut.locator('kbd').allTextContents().then(keys=>keys.join('+')),'Ctrl+R');
    assert.equal(await page.locator('#keybind-label-refreshMetadata').innerText(),'Refresh Anytype lists and pickers');
    assert.equal(await localShortcut.evaluate(node=>node.tagName),'SPAN');
    await page.locator('#keybinds-backdrop').evaluate(node=>Promise.all(node.getAnimations({subtree:true}).map(animation=>animation.finished)));
    await page.screenshot({path:path.join(temp,'keybinds-dark.png')});
    await page.locator('#keybind-button-openSettings').click();
    const capturedRequests=await page.evaluate(()=>window.network);
    await page.keyboard.press('Control+r');
    await page.waitForFunction(()=>window.calls.some(c=>c.name==='save_keyboard_shortcuts'));
    assert.equal(await page.evaluate(()=>window.network),capturedRequests,'Shortcut capture owns Ctrl+R');
    assert.deepEqual(errors,[]);
  }finally{await browser.close();}
});
