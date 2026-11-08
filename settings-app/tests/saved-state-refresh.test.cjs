const test=require('node:test');
const assert=require('node:assert/strict');
const path=require('node:path');
const fs=require('node:fs');
const { loadSettings, openFormulas } = require('./frontend-fixture.cjs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
const workspace=path.resolve(__dirname,'../..');
test('saved-state refresh reconciles Codex edits, dialogs, revisions, failures and queued events',async()=>{
  const temp=path.join(workspace,'.test-run/saved-state/temp');fs.mkdirSync(temp,{recursive:true});process.env.TEMP=process.env.TMP=temp;
  const browser=await chromium.launch({executablePath:process.env.CHROME_PATH||'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true});
  const page=await browser.newPage({colorScheme:'dark',viewport:{width:1050,height:1000}});
  const errors=[];page.on('pageerror',e=>errors.push(e.message));
  await page.addInitScript(()=>{
    const target=id=>({spaceId:'s',objectId:'o',blockId:'b',viewId:id});
    window.calls=[];window.listeners={};window.snapshots=0;
    window.remote={app:{ready:true,theme:'dark',onboardingComplete:true,hasSavedApiKey:true,apiIp:'127.0.0.1',apiPort:31009,settings:{fontFamily:'Inter',reducedMotion:false}},updater:{installDirectory:'fixture',automaticChecksEnabled:false},
      formulas:{configuration:{version:2,display:'balanced',definitions:[{target:target('a'),tokens:[{kind:'number',value:'1'}]}]},revision:'r1'},
      calendar:{version:1,revision:'c1',types:[{spaceId:'s',typeKey:'task',default:{mode:'property',key:'status'},overrides:[]}]}};
    window.__TAURI__={event:{listen:async(name,cb)=>{window.listeners[name]=cb;return()=>{};}},core:{invoke:async(name,args={})=>{
      window.calls.push({name,args});
      if(name==='get_saved_state_snapshot'){
        window.snapshots++;if(window.holdSnapshot)await new Promise(resolve=>window.releaseSnapshot=resolve);
        if(window.failSnapshot)throw Error('fixture read failure');return structuredClone(window.remote);
      }
      if(name==='get_app_state')return structuredClone(window.remote.app);
      if(name==='get_updater_state')return structuredClone(window.remote.updater);
      if(name==='load_kanban_formulas')return structuredClone(window.remote.formulas);
      if(name==='load_calendar_rules')return structuredClone(window.remote.calendar);
      if(name==='get_calendar_api_availability')return 'available';
      if(name==='discover_kanban_views')return {views:['a','b'].map(id=>({target:target(id),name:'Board '+id,space:'Work',source:'Tasks',kind:'Kanban',properties:[]})),info:[]};
      if(name==='discover_calendar_colors')return {types:[{spaceId:'s',key:'task',name:'Task',space:'Work',properties:[{key:'status',name:'Status',format:'select'},{key:'priority',name:'Priority',format:'select'}]}],views:[]};
      if(name==='discover_codex_spaces')return [];
      if(name==='discover_anyblob_types')return {types:[],errors:[]};
      if(name==='discover_codex_sources')return [];
      if(name==='get_keyboard_shortcuts')return [];
      if(name==='save_kanban_formulas'){
        window.lastSave=args;if(window.holdSave)await new Promise(resolve=>window.releaseSave=resolve);
        window.remote.formulas={configuration:args.configuration,revision:'r-saved'};return {revision:'r-saved'};
      }
      if(name==='save_settings'){window.remote.app.settings=args.settings;return {calendarStatus:'off'};}
      if(name==='save_calendar_rules') {window.lastCalendarSave=args;window.remote.calendar={...args.configuration,revision:'c-saved'};return {revision:'c-saved'};}
      return null;
    }}};
    window.publish=()=>window.listeners['blobui-saved-state-changed']({payload:{sections:['formulas','calendarRules'],revision:window.remote.formulas.revision}});
  });
  const settle=async()=>{await page.waitForFunction(()=>!document.getElementById('save-button').disabled || document.getElementById('save-button').hidden);await page.evaluate(()=>window.BlobUI.AppReload.run());};
  try{
    await loadSettings(page);
    await page.waitForFunction(()=>window.appReady&&window.snapshots>0&&window.BlobUI.FormulaPage.read().definitions.length===1);
    await settle();
    await page.locator('#accessibility-nav').click();
    await page.locator('#reduced-motion').check();
    await openFormulas(page);await page.getByRole('button',{name:'Board a',exact:false}).first().click();
    const dialog=page.locator('#formula-calculator-dialog');
    await dialog.getByRole('button',{name:'Show LaTeX',exact:true}).click();
    const source=dialog.getByRole('textbox',{name:'LaTeX source'});
    await source.fill('7');
    await page.evaluate(()=>{window.remote.formulas.configuration.definitions[0].tokens=[{kind:'number',value:'9'}];window.remote.formulas.revision='r2';window.remote.calendar.types[0].default={mode:'property',key:'priority'};window.remote.calendar.revision='c2';window.publish();});
    await page.waitForFunction(()=>window.BlobUI.FormulaPage.read().definitions[0].tokens[0].value==='9');
    assert.equal(await source.inputValue(),'9');assert.equal(await dialog.isVisible(),true);
    assert.equal(await page.locator('#reduced-motion').isChecked(),true);
    assert.equal(await page.locator('#log-entries .log-entry.warning').count(),1);
    assert.equal(await page.evaluate(()=>window.BlobUI.CalendarPage.read().types[0].default.key),'priority');
    // A failed snapshot preserves editor and baseline; retry then loads it.
    await source.fill('10');
    await page.evaluate(()=>{window.failSnapshot=true;window.remote.formulas.configuration.definitions[0].tokens=[{kind:'number',value:'11'}];window.remote.formulas.revision='r3';window.publish();});
    await page.waitForFunction(()=>document.getElementById('log-entries').textContent.includes('fixture read failure'));
    assert.equal(await source.inputValue(),'10');
    await page.evaluate(()=>{window.failSnapshot=false;});await page.keyboard.press('Control+r');
    await page.waitForFunction(()=>window.BlobUI.FormulaPage.read().definitions[0].tokens[0].value==='11');assert.equal(await source.inputValue(),'11');
    // Incomplete source is also a conflicting draft, even though it cannot serialize.
    await source.fill('\\sqrt{');
    await page.evaluate(()=>{window.remote.formulas.configuration.definitions[0].tokens=[{kind:'number',value:'12'}];window.remote.formulas.revision='r4';window.publish();});
    await page.waitForFunction(()=>window.BlobUI.FormulaPage.read().definitions[0].tokens[0].value==='12');assert.equal(await source.inputValue(),'12');
    await source.fill('13');await dialog.getByRole('button',{name:'Save calculation',exact:true}).click();
    await page.locator('#save-button').click();await page.waitForFunction(()=>window.lastSave);
    assert.equal(await page.evaluate(()=>window.lastSave.revision),'r4');
    await settle();
    // Events during a save queue until the exclusive operation finishes.
    await page.getByRole('button',{name:'Board a',exact:false}).first().click();
    if(await dialog.getByRole('button',{name:'Show LaTeX',exact:true}).count()) await dialog.getByRole('button',{name:'Show LaTeX',exact:true}).click();
    await source.fill('14');
    await dialog.getByRole('button',{name:'Save calculation',exact:true}).click();
    await page.evaluate(()=>{window.holdSave=true;});await page.locator('#save-button').click();
    await page.waitForFunction(()=>!!window.releaseSave);
    const count=await page.evaluate(()=>window.snapshots);
    await page.evaluate(()=>window.publish());assert.equal(await page.evaluate(()=>window.snapshots),count);
    await page.evaluate(()=>{window.holdSave=false;window.releaseSave();});
    await page.waitForFunction(n=>window.snapshots>n,count);
    // Undo now restores the newest saved version, including Codex calendar rules.
    await page.evaluate(()=>{window.BlobUI.FormulaPage.restore({version:2,display:'spacious',definitions:[]});window.BlobUI.discardChanges();});
    assert.equal(await page.evaluate(()=>window.BlobUI.FormulaPage.read().definitions[0].tokens[0].value),'14');
    assert.equal(await page.evaluate(()=>window.BlobUI.CalendarPage.read().types[0].default.key),'priority');
    // Open calendar editors use the same conflict policy and keep the dialog open.
    await page.getByRole('button',{name:'Board a',exact:false}).first().click();
    if(await dialog.getByRole('button',{name:'Show LaTeX',exact:true}).count()) await dialog.getByRole('button',{name:'Show LaTeX',exact:true}).click();
    await source.fill('15');await source.fill('\\sqrt{');
    const warningCount=await page.locator('#log-entries .log-entry.warning').count();
    await page.evaluate(()=>{window.remote.formulas.calculatedAt='2026-11-02T18:00:00Z';window.publish();});
    await page.evaluate(()=>window.BlobUI.AppReload.run());
    assert.equal(await source.inputValue(),'\\sqrt{','Results-only publications preserve incomplete source');
    await page.evaluate(()=>{window.remote.formulas.configuration.definitions=[];window.remote.formulas.revision='r-deleted';window.publish();});
    await dialog.waitFor({state:'hidden'});
    assert.equal(await page.locator('#log-entries .log-entry.warning').count(),warningCount+1,'Deletion of an incomplete edited calculation logs one conflict');
    await page.locator('#calendar-events-nav').click();
    await page.locator('#calendar-events-tab-1').click();
    await page.locator('#calendar-type-list').getByRole('button',{name:'Task',exact:false}).first().click();
    const calendarDialog=page.locator('#calendar-rule-dialog');
    const property=calendarDialog.getByRole('button',{name:'Default color property',exact:true});
    await property.click();await calendarDialog.getByRole('option',{name:'Status',exact:true}).click();
    await page.evaluate(()=>{window.remote.calendar.types[0].default={mode:'none'};window.remote.calendar.revision='c3';window.publish();});
    await page.waitForFunction(()=>window.BlobUI.CalendarPage.read().types[0].default.mode==='none');
    assert.equal(await calendarDialog.isVisible(),true);assert.match(await property.innerText(),/No color/);
    await property.click();await calendarDialog.getByRole('option',{name:'Priority',exact:true}).click();
    await calendarDialog.getByRole('button',{name:'Close color rules',exact:true}).click();
    await page.locator('#save-button').click();await page.waitForFunction(()=>window.lastCalendarSave);
    assert.equal(await page.evaluate(()=>window.lastCalendarSave.revision),'c3');await settle();
    // Focus recovery loads missed publications and all saved settings without publishing.
    await page.evaluate(()=>{window.remote.app.settings.fontFamily='Arial';window.remote.updater.autoplayVideos=false;window.remote.app.apiPort=32000;window.dispatchEvent(new Event('focus'));});
    await page.waitForFunction(()=>document.getElementById('font-family').value==='Arial');
    assert.equal(await page.locator('#advanced-api-port').inputValue(),'32000');
    // Repeated events coalesce, with one follow-up to catch commits during a read.
    await page.evaluate(()=>{window.holdSnapshot=true;window.publish();});
    await page.waitForFunction(()=>!!window.releaseSnapshot);
    const inFlight=await page.evaluate(()=>window.snapshots);
    await page.evaluate(()=>{window.publish();window.publish();});
    assert.equal(await page.evaluate(()=>window.snapshots),inFlight);
    await page.evaluate(()=>{window.holdSnapshot=false;window.releaseSnapshot();});
    await page.waitForFunction(n=>window.snapshots===n+1,inFlight);
    // Removing the saved type closes only its editor and Undo uses the new baseline.
    await page.locator('#calendar-type-list').getByRole('button',{name:'Task',exact:false}).first().click();
    await page.evaluate(()=>{window.remote.calendar={version:1,types:[],revision:'c4'};window.publish();});
    await calendarDialog.waitFor({state:'hidden'});
    await page.evaluate(()=>window.BlobUI.discardChanges());
    assert.deepEqual(await page.evaluate(()=>window.BlobUI.CalendarPage.read().types),[]);
    assert.deepEqual(errors,[]);
    assert.equal(await page.evaluate(()=>window.calls.some(c=>/^evaluate_|refresh_kanban_results/.test(c.name))),false);
  }finally{await browser.close();}
});
