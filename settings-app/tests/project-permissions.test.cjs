const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { loadSettings } = require('./frontend-fixture.cjs');
const {chromium} = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const vm = require('node:vm');
const workspace = path.resolve(__dirname,'../..');

test('opening local projects while settings load defers discovery until the controller unlocks', async()=>{
  const source=fs.readFileSync(path.join(workspace,'settings-app/ui/project-permissions.js'),'utf8');
  const window={BlobUI:{}};
  window.window=window;
  vm.runInNewContext(source,{window});
  const controller=window.BlobUI.ProjectPermissions.createController();
  const calls=[];
  controller.init({
    invoke:async name=>{calls.push(name);return [{id:'local',name:'Local project',rootPaths:['C:\\work']}];},
    normalize:value=>value,
    read:()=>({projects:{}}),
    readSaved:()=>({projects:{}}),
    onChange:()=>{},
  });
  controller.open();
  assert.deepEqual(calls,[],'A project page opened before settings unlock does not issue a partial scan.');
  controller.setLocked(false);
  await new Promise(resolve=>setImmediate(resolve));
  assert.deepEqual(calls,['discover_codex_projects']);
  assert.equal(controller.getSnapshot().projects[0].id,'local');
});

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
    await loadSettings(page);await page.waitForFunction(()=>window.appReady);
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
    assert.equal(await dialog.getByText('Enable content changes',{exact:true}).count(),0);
    for (const kind of ['Object types','Collections','Queries']) {
      assert.equal(await sourceRows.filter({has:page.locator('input[aria-label*=" · '+kind+' · "]')}).locator('input[role=switch]').count(),kind==='Queries'?2:1);
    }
    const space=sourceRows.filter({has:page.locator('input[aria-label="Select Work · Spaces · Work"]')});
    const accessX = row => row.locator('.codex-space-bottom button[aria-label^="Access"]').evaluate(node=>node.getBoundingClientRect().x);
    const expectedAccessX = await accessX(space);
    for (let index=0;index<await sourceRows.count();index++) {
      assert.ok(Math.abs(await accessX(sourceRows.nth(index))-expectedAccessX)<1,'Access pickers retain their column without file switches');
    }
    assert.equal(await space.locator('input[aria-label^="Upload/delete"]').isDisabled(),true);
    await space.locator('.formula-view-name').click();
    assert.equal(await space.locator('input[aria-label^="Upload/delete"]').isChecked(),true);
    await space.locator('input[aria-label^="Upload/delete"]').uncheck();
    await space.locator('input[aria-label^="Upload/delete"]').check();
    await space.locator('.formula-view-name').click();
    // The shared modal scrollbar fades in on real scrolling, then fades out after idle.
    const content=dialog.locator('.project-source-content');
    await content.evaluate(node=>{node.style.maxHeight='180px';});
    await page.waitForFunction(()=>document.querySelector('.project-source-content').classList.contains('has-overflow'));
    await page.waitForFunction(()=>getComputedStyle(document.querySelector('.project-source-content')).getPropertyValue('--page-scrollbar-color').trim()==='rgba(0, 0, 0, 0)');
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
    assert.equal(await query.locator('input[role=switch]').count(),1);
    assert.equal(await query.locator('input[type=checkbox]').first().isChecked(),true);
    await query.locator('.formula-view-name').click();
    assert.equal(await page.locator('#save-button').isVisible(),false);
    await query.locator('.formula-view-name').click();
    assert.equal(await query.locator('.codex-space-bottom button[aria-label^="Access"]').innerText(),'Read Only');
    assert.equal(await query.locator('.codex-space-bottom button[aria-label^="Approval"]').innerText(),'Always Ask');
    assert.equal(await query.locator('input[role=switch]').count(),1);
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
    if(await page.locator('#codex-nav').getAttribute('aria-expanded')!=='true')await page.locator('#codex-nav').click();
    await page.locator('#codex-section-links .v21-section-link').nth(2).click();assert.equal(await page.locator('#codex-panel-2').isVisible(),true);
    await page.screenshot({path:path.join(fixture,'projects-refreshed-dark.png'),fullPage:true});assert.deepEqual(errors,[]);
  } finally {await browser.close();}
});

test('cloud project keys stay isolated and their catalog survives offline reloads', async()=>{
  const fixture=path.join(workspace,'.test-run/cloud-project-permissions');fs.mkdirSync(fixture,{recursive:true});process.env.TEMP=process.env.TMP=fixture;
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:900},colorScheme:'dark'}),errors=[];
    page.on('pageerror',error=>errors.push(error.message));
    await page.addInitScript(()=>{
      const source=(access,approval='alwaysAsk')=>({spaceId:'work',kind:'query',id:'shared-query',name:'Tasks',space:'Work',permission:{access,filesEnabled:false,approval}});
      window.calls=[];window.projectError='Codex cloud catalog is offline';
      window.projects=[
        {id:'duplicate-id',name:'Same',rootPaths:['C:\\local'],kind:'local'},
        {id:'duplicate-id',name:'Same',rootPaths:[],kind:'cloud',hostId:'host-a',computerName:'Cloud A'},
        {id:'duplicate-id',name:'Same',rootPaths:[],kind:'cloud',hostId:'host-b',computerName:'Cloud B'},
      ];
      window.sources=[{spaceId:'work',space:'Work',kind:'query',id:'shared-query',name:'Tasks'}];
      window.cloudProjectHandlers={};
      window.__TAURI__={event:{listen:async(name,handler)=>{window.cloudProjectHandlers[name]=handler;return()=>delete window.cloudProjectHandlers[name];}},core:{invoke:async(name,args)=>{
        calls.push(name);
        if(name==='get_app_state')return {ready:true,onboardingComplete:true,theme:'dark',hasSavedApiKey:true,settings:{codexIntegration:{spaces:{work:{access:'read',approval:'alwaysAsk',filesEnabled:false}},projects:{
          'duplicate-id':{name:'Same',rootPaths:['C:\\local'],sources:[source('read')]},
          'cloud:host-a:duplicate-id':{name:'Same',rootPaths:[],sources:[source('readWrite')]},
          'cloud:host-old:removed-id':{name:'Old cloud',rootPaths:[],sources:[source('createDelete','fullAccess')]},
        }}}};
        if(name==='get_updater_state')return {automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return 'available';
        if(name==='discover_codex_projects'){if(window.projectError)throw Error(window.projectError);return projects;}
        if(name==='discover_codex_sources'){if(window.sourceError)throw Error(window.sourceError);return sources;}
        if(name==='discover_codex_spaces')return [{id:'work',name:'Work'}];
        if(name==='save_settings'){if(window.saveError)throw Error('Fixture save failed');window.saved=JSON.parse(JSON.stringify(args.settings));return {};}
        return null;
      }}};
    });
    await loadSettings(page);await page.waitForFunction(()=>window.appReady);
    await page.locator('#codex-nav').click();await page.locator('#codex-tab-2').click();
    const rows=page.locator('#codex-project-list .formula-view-row');
    await page.getByRole('status').filter({hasText:'Codex cloud catalog is offline'}).waitFor();
    assert.equal(await rows.count(),3,'saved local and cloud overrides remain visible before the first successful catalog read');
    assert.equal(await page.locator('#codex-project-list .formula-view-warning').count(),0,'an offline first scan does not mark saved entries unavailable');

    const cloudHelp=page.getByRole('button',{name:'Cloud project information for Same'}).first();
    await cloudHelp.focus();
    const tooltipId=await cloudHelp.getAttribute('aria-describedby');
    await page.waitForFunction(id=>document.getElementById(id)?.dataset.open==='true'&&document.getElementById(id)?.hasAttribute('data-positioned'),tooltipId);
    assert.match(await page.evaluate(id=>document.getElementById(id).textContent,tooltipId),/host-a|host-old/);
    await cloudHelp.blur();await cloudHelp.hover();await page.waitForFunction(id=>document.getElementById(id)?.dataset.open==='true',tooltipId);
    await page.mouse.move(5,5);await page.waitForFunction(id=>document.getElementById(id)?.dataset.open==='false',tooltipId);

    await page.evaluate(()=>{window.projectError='';window.cloudProjectHandlers['anyblob-cloud-projects-changed']?.({payload:null});});
    await page.waitForFunction(()=>document.querySelectorAll('#codex-project-list .formula-view-row').length===4);
    const keys=await page.locator('#codex-project-list .formula-view-row').evaluateAll(nodes=>nodes.map(node=>node.dataset.projectKey));
    assert.deepEqual(keys.sort(),['cloud:host-a:duplicate-id','cloud:host-b:duplicate-id','cloud:host-old:removed-id','duplicate-id'].sort());
    const stale=page.locator('#codex-project-list .project-catalog-entry').filter({has:page.locator('[data-project-key="cloud:host-old:removed-id"]')});
    await stale.getByText('Unavailable project. Saved selections are retained.').waitFor();

    const queryRow=page.locator('#project-source-list .codex-space').filter({has:page.locator('input[aria-label="Select Tasks · Queries · Work"]')});
    const access=()=>queryRow.locator('.codex-space-bottom button[aria-label^="Access"]');
    await page.locator('#codex-project-list [data-project-key="duplicate-id"]').click();
    await queryRow.waitFor();assert.equal(await access().innerText(),'Read Only');
    await page.locator('button[aria-label="Close project sources"]').click();await page.locator('#project-source-backdrop').waitFor({state:'hidden'});
    await page.locator('#codex-project-list [data-project-key="cloud:host-a:duplicate-id"]').click();
    await queryRow.waitFor();assert.equal(await access().innerText(),'Read-Write');
    await page.locator('button[aria-label="Close project sources"]').click();await page.locator('#project-source-backdrop').waitFor({state:'hidden'});
    await page.locator('#codex-project-list [data-project-key="cloud:host-b:duplicate-id"]').click();
    await queryRow.waitFor();assert.equal(await queryRow.locator('input[type=checkbox][aria-label^="Select"]').isChecked(),false);
    await queryRow.locator('.formula-view-name').click();
    await access().click();await page.locator('#project-source-dialog').getByRole('option',{name:'Create-Delete',exact:true}).click();
    await page.locator('button[aria-label="Close project sources"]').click();await page.locator('#project-source-backdrop').waitFor({state:'hidden'});
    await page.locator('#save-button').click();
    await page.waitForFunction(()=>window.saved?.codexIntegration.projects['cloud:host-b:duplicate-id']?.sources[0]?.permission.access==='createDelete');
    assert.equal(await page.evaluate(()=>saved.codexIntegration.projects['cloud:host-a:duplicate-id'].sources[0].permission.access),'readWrite');
    assert.equal(await page.evaluate(()=>saved.codexIntegration.projects['duplicate-id'].sources[0].permission.access),'read');

    await page.evaluate(()=>{window.projectError='Offline refresh';});await page.locator('#projects-reload').click();
    await page.getByRole('status').filter({hasText:'Offline refresh'}).waitFor();
    assert.equal(await rows.count(),4,'failed reload keeps the last cloud catalog and saved overrides');
    await page.evaluate(()=>{window.projectError='';window.projects=window.projects.filter(project=>project.kind==='local');});
    await page.locator('#projects-reload').click();
    await page.waitForFunction(()=>document.querySelector('#projects-reload')?.getAttribute('aria-busy')==='false');
    assert.equal(await page.locator('#codex-project-list [data-project-key="cloud:host-a:duplicate-id"]').count(),1,'a local-only offline result does not erase cached cloud projects');
    assert.equal(await page.locator('#codex-project-list [data-project-key="cloud:host-b:duplicate-id"]').count(),1);

    await page.evaluate(()=>{window.projects=window.projects.filter(project=>project.kind==='local'||project.hostId==='host-b');window.cloudProjectHandlers['anyblob-cloud-projects-changed']?.({payload:null});});
    await page.waitForFunction(()=>document.querySelector('#codex-project-list [data-project-key="cloud:host-a:duplicate-id"]')?.closest('.project-catalog-entry')?.innerText.includes('Unavailable project'));
    await page.emulateMedia({reducedMotion:'reduce'});await page.setViewportSize({width:620,height:850});
    assert.equal(await page.evaluate(()=>getComputedStyle(document.querySelector('.help-tooltip-popover')).transitionDuration),'0s');
    assert.equal(await page.evaluate(()=>document.querySelector('.page-content').scrollWidth<=document.querySelector('.page-content').clientWidth),true);
    await page.screenshot({path:path.join(fixture,'cloud-projects-narrow-dark.png'),fullPage:true});
    assert.equal(await page.evaluate(()=>window.cloudProjectHandlers['anyblob-cloud-projects-changed']!==undefined),true);
    assert.deepEqual(errors,[]);
  } finally {await browser.close();}
});
