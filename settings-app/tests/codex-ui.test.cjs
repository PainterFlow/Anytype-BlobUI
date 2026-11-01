const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const workspace = path.resolve(__dirname, '../..');
test('Codex permissions preserve drafts, save and undo in both layouts', async () => {
  const root = path.join(workspace,'.test-run/codex-ui'); fs.mkdirSync(root,{recursive:true});
  process.env.TEMP=process.env.TMP=root;
  const browser = await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:900},colorScheme:'dark'}), errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(()=>{
      window.calls=[];window.holdDiscovery=true;window.spaces=[{id:'full-space-one',name:'Work'},{id:'full-space-two',name:'Work'},{id:'long-space',name:'A very long space name '.repeat(12)},...Array.from({length:25},(_,i)=>({id:'many-'+i,name:'Space '+i}))];
      window.__TAURI__={core:{invoke:async(name,args)=>{
        window.calls.push(name);
        if(name==='get_app_state')return {ready:true,onboardingComplete:true,theme:'dark',hasSavedApiKey:true,settings:{codexIntegration:{spaces:{absent:{access:'read',filesEnabled:true,approval:'fullAccess'}}}}};
        if(name==='get_updater_state')return {automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return 'available';
        if(name==='discover_codex_spaces'){if(window.holdDiscovery)await new Promise(resolve=>{(window.pendingDiscovery||=[]).push(resolve);window.resolveDiscovery=()=>window.pendingDiscovery.splice(0).forEach(done=>done());});if(window.discoveryError)throw Error(window.discoveryError);return window.spaces;}
        if(name==='discover_anyblob_types')return{types:[],errors:[]};
        if(name==='save_settings'){if(window.saveError)throw Error('Save failed');window.saved=args.settings;return {};}
        return null;
      }}};
    });
    await page.goto(pathToFileURL(path.join(workspace,'settings-app/ui/index.html')).href);
    await page.waitForFunction(()=>window.appReady);
    await page.locator('#codex-nav').click();
    assert.equal(await page.locator('#codex-panel-0').getAttribute('hidden'),null);
    assert.match(await page.locator('#codex-panel-0').innerText(),/Enable Anyblob/);
    assert.equal(await page.evaluate(()=>calls.filter(c=>c==='discover_codex_spaces').length),1);
    await page.locator('#codex-tab-0').focus();await page.keyboard.press('ArrowRight');
    await page.locator('#codex-panel-1').getByRole('status').filter({hasText:'Loading spaces'}).waitFor();
    await page.evaluate(()=>{window.holdDiscovery=false;window.resolveDiscovery();});
    await page.locator('.codex-space').first().waitFor();
    assert.deepEqual(await page.evaluate(()=>window.BlobUI.CodexPage.normalize({spaces:{invalid:{access:'bad',approval:4,filesEnabled:'yes'}}})),{spaces:{invalid:{access:'none',filesEnabled:false,approval:'approveForMe'}},projects:{},objectMentions:{spaces:{all:true,items:[]},types:{all:true,items:[]}}});
    assert.deepEqual(await page.evaluate(()=>window.BlobUI.CodexPage.normalize({objectMentions:null}).objectMentions),{spaces:{all:false,items:[]},types:{all:false,items:[]}});
    const filters={spaces:{all:false,items:[{id:'unavailable',name:'Saved'}]},types:{all:false,items:[{spaceId:'other',id:'missing',name:'Task',space:'Old'}]}};
    assert.deepEqual(await page.evaluate(filters=>window.BlobUI.CodexPage.normalize({objectMentions:filters}).objectMentions,filters),filters);
    const row=page.locator('.codex-space').first(), access=row.locator('.codex-space-bottom button[aria-label^="Access"][aria-haspopup]'), approval=row.locator('.codex-space-bottom button[aria-label^="Approval"]');
    assert.equal(await access.innerText(),'No Access');assert.equal(await approval.innerText(),'Approve for me');
    assert.equal(await row.locator('input').isChecked(),false);
    await row.locator('input').check();
    await access.focus();await page.keyboard.press('ArrowDown');await page.keyboard.press('End');await page.keyboard.press('Enter');
    assert.equal(await access.innerText(),'Create-Delete');
    await approval.click();await row.getByRole('option',{name:'Always Ask',exact:true}).click();
    await page.locator('#codex-search-toggle').click();await page.locator('#codex-search').fill('no results');
    assert.equal(await page.locator('.codex-space:visible').count(),0);
    await page.locator('#codex-search').fill('Work');assert.equal(await page.locator('.codex-space:visible').count(),2);
    await page.evaluate(()=>window.discoveryError='Fixture reload failed');await page.locator('#codex-reload').click();
    await page.getByRole('status').filter({hasText:'Fixture reload failed'}).waitFor();
    assert.equal(await page.locator('.codex-space:visible').count(),2);assert.equal(await access.innerText(),'Create-Delete');
    await page.locator('#preferences-nav').click();await page.locator('#codex-nav').click();
    assert.equal(await access.innerText(),'Create-Delete');
    await page.evaluate(()=>window.saveError=true);await page.locator('#save-button').click();
    await page.waitForFunction(()=>document.getElementById('status-message').textContent.includes('Save failed'));
    assert.equal(await row.locator('input').isChecked(),true);await page.evaluate(()=>window.saveError=false);
    await page.locator('#save-button').click();await page.waitForFunction(()=>window.saved?.codexIntegration.spaces['full-space-one']?.approval==='alwaysAsk');
    assert.deepEqual(await page.evaluate(()=>window.saved.codexIntegration.spaces.absent),{access:'read',filesEnabled:true,approval:'fullAccess'});
    await row.locator('input').uncheck();await page.evaluate(()=>window.BlobUI.discardChanges());assert.equal(await row.locator('input').isChecked(),true);
    await approval.click();await row.getByRole('option',{name:'Never Ask',exact:true}).click();
    await page.locator('#advanced-nav').click();await page.locator('#advanced-experiments-tab').click();await page.locator('#settings-v21-enabled').check();
    await page.locator('#codex-nav').click();await page.locator('#codex-section-links .v21-section-link').nth(1).click();
    assert.equal(await access.innerText(),'Create-Delete');
    assert.equal(await approval.innerText(),'Never Ask');
    assert.equal(await page.locator('#codex-nav').getAttribute('aria-expanded'),'true');
    await page.locator('#codex-search').fill('');
    await page.locator('#codex-section-links').evaluate(el => Promise.all(el.getAnimations().map(a => a.finished)));
    await page.screenshot({path:path.join(root,'desktop.png'),fullPage:true});
    await page.setViewportSize({width:620,height:850});await page.emulateMedia({reducedMotion:'reduce'});
    await page.screenshot({path:path.join(root,'narrow.png'),fullPage:true});
    assert.equal(await page.evaluate(()=>document.querySelector('.page-content').scrollWidth<=document.querySelector('.page-content').clientWidth),true);
    await page.evaluate(()=>{window.discoveryError='';window.spaces=[];});await page.locator('#codex-reload').click();await page.getByRole('status').filter({hasText:'No accessible spaces found'}).waitFor();
    await page.evaluate(()=>window.discoveryError='Add an Anytype API key in Advanced Settings to list spaces.');await page.locator('#codex-reload').click();await page.getByRole('button',{name:'Open Advanced Settings > API key'}).click();assert.equal(await page.locator('#advanced-api-panel').isVisible(),true);
    assert.deepEqual(errors,[]);
  } finally {await browser.close();}
});
