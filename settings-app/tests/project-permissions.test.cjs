const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const {pathToFileURL} = require('node:url');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const workspace = path.resolve(__dirname,'../..');
test('project catalogs and source drafts survive search, refresh, modal dismissal, Save and Undo in dark layouts', async()=>{
  const fixture=path.join(workspace,'.test-run/project-permissions');fs.mkdirSync(fixture,{recursive:true});process.env.TEMP=process.env.TMP=fixture;
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:900},colorScheme:'dark'}),errors=[];
    page.on('pageerror',error=>errors.push(error.message));
    await page.addInitScript(()=>{
      window.calls=[];window.projects=[{id:'alpha',name:'Same',rootPaths:['C:\\one','C:\\worktree']},{id:'beta',name:'Same',rootPaths:['C:\\two']}];
      window.sources=[{spaceId:'work',space:'Work',kind:'space',id:'work',name:'Work'},
        {spaceId:'work',space:'Work',kind:'type',id:'type-full-id',name:'Task'},
        {spaceId:'work',space:'Work',kind:'collection',id:'collection-id',name:'Tasks'},
        {spaceId:'work',space:'Work',kind:'query',id:'query-id',name:'Tasks'},
        {spaceId:'private',space:'Private',kind:'query',id:'query-id',name:'Tasks'}];
      window.__TAURI__={core:{invoke:async(name,args)=>{
        calls.push(name);
        if(name==='get_app_state')return {ready:true,onboardingComplete:true,theme:'dark',hasSavedApiKey:true,settings:{codexIntegration:{spaces:{work:{access:'read',approval:'alwaysAsk',filesEnabled:true},private:{access:'none',approval:'fullAccess',filesEnabled:false}}}}};
        if(name==='get_updater_state')return {automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return 'available';
        if(name==='discover_codex_projects'){if(window.projectError)throw Error(window.projectError);return projects;}
        if(name==='discover_codex_sources'){if(window.sourceError)throw Error(window.sourceError);return sources;}
        if(name==='discover_codex_spaces')return [{id:'work',name:'Work'}];
        if(name==='save_settings'){if(window.saveError)throw Error('Fixture save failed');window.saved=JSON.parse(JSON.stringify(args.settings));return {};}
        return null;
      }}};
    });
    await page.goto(pathToFileURL(path.join(workspace,'settings-app/ui/index.html')).href);await page.waitForFunction(()=>window.appReady);
    assert.equal(await page.evaluate(()=>calls.includes('discover_codex_projects')),false);
    await page.locator('#codex-nav').click();await page.locator('#codex-tab-2').click();
    const projects=page.locator('#codex-project-list .formula-view-row');await projects.first().waitFor();assert.equal(await projects.count(),2);
    assert.equal(await page.locator('#projects-filter-control').isVisible(),false);
    assert.equal(await projects.first().innerText(),'Same');
    assert.equal(await projects.first().getAttribute('title'),'Same');
    await page.locator('#codex-tab-2').focus();
    assert.equal(await page.locator('#codex-tab-2').evaluate(node=>getComputedStyle(node).outlineStyle),'none');
    await page.locator('#projects-search-toggle').click();await page.locator('#projects-search').fill('worktree');assert.equal(await projects.filter({visible:true}).count(),1);
    await page.evaluate(()=>window.projectError='Fixture project refresh failed');await page.locator('#projects-reload').click();await page.getByRole('status').filter({hasText:'Fixture project refresh failed'}).waitFor();assert.equal(await projects.count(),2);
    await projects.first().click();const dialog=page.locator('#project-source-dialog');await dialog.waitFor();
    const sourceRows=page.locator('#project-source-list .codex-space');await sourceRows.nth(4).waitFor();
    // The shared modal scrollbar fades in on real scrolling, then fades out after idle.
    const content=dialog.locator('.project-source-content');
    await content.evaluate(node=>{node.style.maxHeight='180px';});
    await page.waitForFunction(()=>document.querySelector('.project-source-content').classList.contains('has-overflow'));
    assert.equal(await content.evaluate(node=>getComputedStyle(node).getPropertyValue('--page-scrollbar-color').trim()),'rgba(0, 0, 0, 0)');
    await content.evaluate(node=>{node.scrollTop=40;});
    await page.waitForFunction(()=>document.querySelector('.project-source-content').getAnimations().some(animation=>animation.transitionProperty==='--page-scrollbar-color'));
    await page.waitForFunction(()=>{const node=document.querySelector('.project-source-content');return !node.classList.contains('is-scrolling')&&node.getAnimations().some(animation=>animation.transitionProperty==='--page-scrollbar-color');});
    await page.waitForFunction(()=>getComputedStyle(document.querySelector('.project-source-content')).getPropertyValue('--page-scrollbar-color').trim()==='rgba(0, 0, 0, 0)');
    await content.evaluate(node=>{node.style.maxHeight='';node.scrollTop=0;});
    const query=sourceRows.filter({has:page.locator('input[aria-label="Select Tasks · Queries · Work"]')});
    assert.equal(await query.locator('.codex-space-bottom button[aria-label^="Access"]').isDisabled(),true);
    const disabledAccess=query.locator('.codex-space-bottom button[aria-label^="Access"]');
    const disabledStyle=await disabledAccess.evaluate(node=>({color:getComputedStyle(node).color,background:getComputedStyle(node).backgroundColor}));
    await disabledAccess.hover();
    assert.deepEqual(await disabledAccess.evaluate(node=>({color:getComputedStyle(node).color,background:getComputedStyle(node).backgroundColor})),disabledStyle);
    assert.equal(disabledStyle.color,'rgb(102, 102, 102)');
    await query.locator('.formula-view-name').click();
    await query.locator('.settings-row > span').first().click();
    assert.equal(await query.locator('input[role=switch]').isChecked(),false);
    assert.equal(await query.locator('input[type=checkbox]').first().isChecked(),true);
    await query.locator('.formula-view-name').click();
    assert.equal(await page.locator('#save-button').isVisible(),false);
    await query.locator('.formula-view-name').click();
    assert.equal(await query.locator('.codex-space-bottom button[aria-label^="Access"]').innerText(),'Read Only');
    assert.equal(await query.locator('.codex-space-bottom button[aria-label^="Approval"]').innerText(),'Always Ask');
    assert.equal(await query.locator('input[role=switch]').isChecked(),true);
    assert.equal(await sourceRows.first().getAttribute('data-selected'),'true');
    assert.equal(await query.evaluate(node=>getComputedStyle(node).outlineWidth),'2px');
    assert.equal(await query.locator('.project-source-metadata').innerText(),'Queries · Work');
    assert.equal(await query.locator('.formula-view-name').getAttribute('title'),'Tasks');
    assert.equal(await query.locator('input[type=checkbox]').first().evaluate(node=>getComputedStyle(node).clipPath),'inset(50%)');
    // Equal names in another space retain independent identities and permissions.
    const privateQuery=sourceRows.filter({has:page.locator('input[aria-label="Select Tasks · Queries · Private"]')});
    await privateQuery.locator('.formula-view-name').click();assert.equal(await privateQuery.locator('.codex-space-bottom button[aria-label^="Access"]').innerText(),'No Access');
    await privateQuery.locator('.codex-space-bottom button[aria-label^="Access"]').click();await dialog.getByRole('option',{name:'Read-Write',exact:true}).click();
    // Multiple categories remain selected and the filter stays open.
    await page.locator('#project-sources-filter').click();await dialog.getByRole('option',{name:'Spaces',exact:true}).click();
    assert.equal(await page.locator('#project-sources-filter').getAttribute('aria-expanded'),'true');
    assert.equal(await dialog.getByRole('option',{name:'Object types',exact:true}).getAttribute('aria-selected'),'true');
    await dialog.getByRole('option',{name:'Collections',exact:true}).click();await page.keyboard.press('Escape');
    assert.equal(await dialog.isVisible(),true);
    assert.equal(await sourceRows.filter({visible:true}).count(),3);
    await page.locator('#project-sources-search-toggle').click();await page.locator('#project-sources-search').fill('Private');assert.equal(await sourceRows.filter({visible:true}).count(),1);
    await page.evaluate(()=>window.sourceError='Fixture source refresh failed');await page.locator('#project-sources-reload').click();await page.getByRole('status').filter({hasText:'Fixture source refresh failed'}).waitFor();
    assert.equal(await privateQuery.locator('.codex-space-bottom button[aria-label^="Access"]').innerText(),'Read-Write');
    await page.locator('#project-sources-search').fill('');
    // Escape and backdrop dismissal preserve selections and the shared draft.
    await page.locator('button[aria-label="Close project sources"]').click();await page.locator('#project-source-backdrop').waitFor({state:'hidden'});
    await projects.first().click();assert.equal(await query.locator('input[type=checkbox]').first().isChecked(),true);
    await page.locator('button[aria-label="Close project sources"]').focus();await page.keyboard.press('Shift+Tab');assert.equal(await page.evaluate(()=>!!document.activeElement.closest('#project-source-dialog')),true);
    await page.keyboard.press('Escape');await page.locator('#project-source-backdrop').waitFor({state:'hidden'});
    await page.evaluate(()=>{window.projectError='';window.projects[0].name='Renamed';});await page.locator('#projects-reload').click();await projects.filter({hasText:'Renamed'}).waitFor();
    await page.locator('#save-button').click();await page.waitForFunction(()=>window.saved?.codexIntegration.projects.alpha?.sources.length===2);
    assert.equal(await page.evaluate(()=>saved.codexIntegration.projects.alpha.sources.find(row=>row.spaceId==='private').permission.access),'readWrite');
    assert.equal(await page.evaluate(()=>saved.codexIntegration.spaces.private.access),'none');
    // Global changes cannot mutate an independent saved override.
    await page.locator('#codex-tab-1').click();const global=page.locator('#codex-panel-1 .codex-space').first();await global.waitFor();
    await global.locator('.codex-space-bottom button[aria-label^="Access"][aria-haspopup]').click();await page.locator('#codex-panel-1').getByRole('option',{name:'No Access',exact:true}).click();
    await page.locator('#codex-tab-2').click();await projects.filter({hasText:'Renamed'}).click();
    assert.equal(await query.locator('.codex-space-bottom button[aria-label^="Access"]').innerText(),'Read Only');
    await query.locator('input[type=checkbox]').first().focus();await page.keyboard.press('Space');await query.locator('.formula-view-name').click();
    assert.equal(await query.locator('.codex-space-bottom button[aria-label^="Access"]').innerText(),'No Access');
    await page.locator('button[aria-label="Close project sources"]').click();await page.locator('#project-source-backdrop').waitFor({state:'hidden'});
    await page.evaluate(()=>window.BlobUI.discardChanges());await projects.filter({hasText:'Renamed'}).click();assert.equal(await query.locator('.codex-space-bottom button[aria-label^="Access"]').innerText(),'Read Only');
    // Only successful scans mark saved sources unavailable, while retaining them.
    await page.evaluate(()=>{window.sourceError='';window.sources=[];});await page.locator('#project-sources-reload').click();await dialog.getByText('Unavailable source. Its saved selection is retained.').first().waitFor();
    assert.equal(await query.locator('input[type=checkbox]').first().isChecked(),true);
    await page.evaluate(()=>Promise.all(document.getAnimations().map(animation=>animation.finished.catch(()=>{}))));
    await page.screenshot({path:path.join(fixture,'sources-dark.png'),fullPage:true});
    await page.setViewportSize({width:620,height:850});await page.emulateMedia({reducedMotion:'reduce'});
    assert.equal(await content.evaluate(node=>getComputedStyle(node).transitionDuration),'0s');
    assert.equal(await dialog.evaluate(node=>node.scrollWidth<=node.clientWidth),true);
    await page.screenshot({path:path.join(fixture,'sources-narrow-dark.png'),fullPage:true});
    await page.locator('button[aria-label="Close project sources"]').click();
    await page.evaluate(()=>window.projects=[]);await page.locator('#projects-reload').click();await page.locator('#codex-project-list').getByText('Unavailable project. Saved selections are retained.').waitFor();
    await page.locator('#advanced-nav').click();await page.locator('#advanced-experiments-tab').click();await page.locator('#settings-v21-enabled').check();await page.locator('#codex-nav').click();
    await page.locator('#codex-section-links .v21-section-link').nth(2).click();assert.equal(await page.locator('#codex-panel-2').isVisible(),true);
    await page.screenshot({path:path.join(fixture,'projects-refreshed-dark.png'),fullPage:true});assert.deepEqual(errors,[]);
  } finally {await browser.close();}
});
