const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const { loadSettings } = require('./frontend-fixture.cjs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
test('Anyblob setup has no approval listeners in a dark isolated fixture',async()=>{
  const workspace=path.resolve(__dirname,'../..'),fixture=path.join(workspace,'.test-run/anyblob-ui');fs.mkdirSync(fixture,{recursive:true});process.env.TEMP=process.env.TMP=fixture;
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:900},colorScheme:'dark'}),errors=[];page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(()=>{
      window.events={};window.decisions=[];window.enabled=false;window.localProjectsFolder='';window.installed=false;window.installCalls=0;
      window.__TAURI__={event:{listen:async(name,callback)=>{window.events[name]=callback;return()=>{};}},core:{invoke:async(name,args)=>{
        if(name==='get_app_state')return{ready:true,onboardingComplete:true,theme:'dark',settings:{}};
        if(name==='get_updater_state')return{automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return'available';
        if(name==='anyblob_status'){if(window.statusError)throw Error(window.statusError);return{enabled:window.enabled,localProjectsFolder:window.localProjectsFolder};}
        if(name==='configure_anyblob'){if(window.toggleError)throw Error('Could not save Anyblob.');window.enabled=args.enabled;return{enabled:args.enabled};}
        if(name==='select_anyblob_projects_folder')return window.folderSelection || null;
        if(name==='set_anyblob_projects_folder'){window.localProjectsFolder=args.folder;return{localProjectsFolder:args.folder};}
        if(name==='secure_mcp_tunnel_status')return window.tunnel || {enabled:false,tunnelId:'',keyConfigured:false,runtimeAvailable:true,running:false,ready:false};
        if(name==='configure_secure_mcp_tunnel'){
          window.tunnel={enabled:args.enabled,tunnelId:args.tunnelId,keyConfigured:!!args.runtimeApiKey||!!window.tunnel?.keyConfigured,runtimeAvailable:true,running:!!args.start,ready:!!args.start};
          return null;
        }
        if(name==='anyblob_installation_status')return{installed:window.installed,updateAvailable:!!window.updateAvailable,codexRefreshRequired:!!window.codexRefreshRequired,bundledVersion:'0.1.1',installedVersion:window.installed?(window.installedVersion||'0.1.0'):null};
        if(name==='install_anyblob_marketplace'){window.installCalls++;await new Promise(resolve=>window.finishInstall=resolve);if(window.installError)throw Error('Fixture install failed');window.installed=true;return null;}
        return null;
      }}};
    });
    await loadSettings(page);await page.waitForFunction(()=>window.appReady);await page.locator('#codex-nav').click();
    const panel=page.locator('#codex-panel-0'),toggle=panel.locator('input[type=checkbox]').first(),install=panel.getByRole('button',{name:'Install',exact:true});
    assert.equal(await page.locator('#codex-nav .nav-label').count() ? await page.locator('#codex-nav .nav-label').innerText() : await page.locator('#codex-nav').innerText(),'Anyblob');
    assert.deepEqual(await panel.locator('h2').allTextContents(),['General','Object Mentions']);assert.equal(await panel.locator('.settings-list > .settings-row').count(),7);
    assert.equal(await panel.getByRole('heading',{name:'Remote Codex',exact:true}).count(),0);
    const integrationGroups=panel.locator('.anyblob-integration-settings');assert.equal(await integrationGroups.count(),3);
    const firstIntegrationGroup=await integrationGroups.nth(0).boundingBox(),secondIntegrationGroup=await integrationGroups.nth(1).boundingBox();
    assert(secondIntegrationGroup.y-firstIntegrationGroup.y-firstIntegrationGroup.height>=16);
    assert.equal(await panel.locator('#enable-anyblob').evaluate(el=>getComputedStyle(el.closest('label').querySelector('span:first-child')).fontSize),'16px');
    assert.equal(await panel.locator('.object-mention-row .kanban-setting-title').first().evaluate(el=>getComputedStyle(el).fontSize),'16px');
    assert.equal(await panel.locator('.kanban-setting-title').first().evaluate(el=>getComputedStyle(el).fontSize),'16px');
    const helperLabelSizes=await page.evaluate(()=>{
      const measure=(className,copyClass,helperClass,titleClass,helperVisible)=>{
        const row=document.createElement('button');row.className=className;
        const copy=copyClass?document.createElement('span'):row;if(copyClass)copy.className=copyClass;
        const title=document.createElement('span');title.className=titleClass;title.textContent='Title';copy.append(title);
        if(helperClass){const helper=document.createElement('span');helper.className=helperClass;helper.textContent='Helper text';if(!helperVisible)helper.hidden=true;copy.append(helper);}
        if(copyClass)row.append(copy);
        document.body.append(row);const size=getComputedStyle(title).fontSize;row.remove();return size;
      };
      return {
        settingWithoutHelper:measure('settings-row kanban-setting-row','kanban-setting-copy','settings-row-description','kanban-setting-title',false),
        settingWithHelper:measure('settings-row kanban-setting-row','kanban-setting-copy','settings-row-description','kanban-setting-title',true),
        optionWithoutHelper:measure('kanban-select-option','kanban-option-copy','kanban-select-option-description','kanban-option-label',false),
        optionWithHelper:measure('kanban-select-option','kanban-option-copy','kanban-select-option-description','kanban-option-label',true),
        warningWithoutHelper:measure('formula-view-row',null,'formula-view-warning','formula-view-name',false),
        warningWithHelper:measure('formula-view-row',null,'formula-view-warning','formula-view-name',true),
      };
    });
    assert.deepEqual(helperLabelSizes,{
      settingWithoutHelper:'16px',settingWithHelper:'14px',
      optionWithoutHelper:'16px',optionWithHelper:'14px',
      warningWithoutHelper:'16px',warningWithHelper:'14px',
    });
    const saveBusyMessage='Wait for the current save before changing Anyblob.';
    await page.evaluate(message=>{window.statusError=message;window.dispatchEvent(new Event('blobui-codex-integration-refresh'));},saveBusyMessage);
    await page.waitForFunction(message=>document.querySelector('#anyblob-integration-status')?.textContent===message,saveBusyMessage);
    await page.evaluate(()=>{window.statusError=null;window.dispatchEvent(new Event('blobui-codex-integration-refresh'));});
    await page.waitForFunction(()=>document.querySelector('#anyblob-integration-status')?.textContent==='');
    const folder = panel.getByRole('button', { name: 'Local Projects Folder', exact: true });
    const tunnelToggle = panel.getByRole('switch', { name: 'Secure MCP Tunnel' });
    const tunnelLabel = panel.getByRole('button', { name: 'Secure MCP Tunnel' });
    const tunnelDialog = page.getByRole('dialog', { name: 'Secure MCP Tunnel' });
    const startTunnel = tunnelDialog.getByRole('button', { name: 'Start tunnel' });
    const save = page.locator('#save-button');
    const undoDraft = async () => {
      await page.locator('#discard-button').click();
      await page.locator('#discard-confirmation-undo').click();
      await save.waitFor({ state: 'hidden' });
    };
    const closeTunnel = async () => {
      await page.keyboard.press('Escape');
      await tunnelDialog.waitFor({ state: 'hidden' });
    };
    assert.equal(await toggle.isChecked(), false);
    assert.equal(await folder.isDisabled(), true);
    assert.equal(await tunnelToggle.isDisabled(), true);
    await toggle.check();
    assert.equal(await page.evaluate(() => window.enabled), false);
    await save.click();
    await save.waitFor({ state: 'hidden' });
    assert.equal(await page.evaluate(() => window.enabled), true);
    await folder.click();
    assert.equal(await folder.inputValue(), '');
    await page.evaluate(() => { window.folderSelection = 'C:/Fixture Projects'; });
    await folder.click();
    assert.equal(await folder.inputValue(), 'C:/Fixture Projects');
    assert.equal(await folder.evaluate(el => getComputedStyle(el).caretColor), 'rgba(0, 0, 0, 0)');
    assert.equal(await page.evaluate(() => window.localProjectsFolder), '');
    await undoDraft();
    assert.equal(await folder.inputValue(), '');
    await folder.click();
    await save.click();
    await save.waitFor({ state: 'hidden' });
    assert.equal(await page.evaluate(() => window.localProjectsFolder), 'C:/Fixture Projects');
    await tunnelLabel.click();
    await tunnelDialog.waitFor();
    const closeButton = tunnelDialog.getByRole('button', { name: 'Close Secure MCP Tunnel' });
    assert.equal(await closeButton.evaluate(el => getComputedStyle(el).lineHeight), '24px');
    assert.equal(await tunnelToggle.isChecked(), false);
    const tunnelId = tunnelDialog.getByLabel('Tunnel ID', { exact: true });
    const key = tunnelDialog.getByLabel('Runtime API key', { exact: true });
    assert.equal(await tunnelId.isDisabled(), true);
    await closeTunnel();
    await tunnelToggle.check();
    await tunnelDialog.waitFor();
    assert.equal(await startTunnel.isDisabled(), true);
    await tunnelId.fill('tunnel_fixture');
    assert.equal(await key.getAttribute('type'), 'password');
    await key.fill('fixture-runtime-key');
    assert.equal(await tunnelId.getAttribute('type'), 'password');
    await tunnelDialog.getByRole('button', { name: 'Show Tunnel ID' }).click();
    assert.equal(await tunnelId.getAttribute('type'), 'text');
    await tunnelDialog.getByRole('button', { name: 'Show Runtime API key' }).click();
    assert.equal(await key.getAttribute('type'), 'text');
    await closeTunnel();
    await tunnelToggle.uncheck();
    assert.equal(await tunnelDialog.isVisible(), false);
    await undoDraft();
    await tunnelToggle.check();
    await tunnelDialog.waitFor();
    assert.equal(await tunnelId.inputValue(), '');
    assert.equal(await key.inputValue(), '');
    await tunnelId.fill('tunnel_fixture');
    await key.fill('fixture-runtime-key');
    await tunnelDialog.getByRole('button', { name: 'Show Runtime API key' }).click();
    await closeTunnel();
    await tunnelLabel.click();
    await tunnelDialog.waitFor();
    assert.equal(await key.getAttribute('type'), 'password');
    assert.equal(await key.inputValue(), 'fixture-runtime-key');
    await page.screenshot({ path: path.join(fixture, 'secure-tunnel-dialog-dark.png'), animations: 'disabled' });
    await closeTunnel();
    assert.equal(await page.evaluate(() => window.tunnel), undefined);
    await save.click();
    await save.waitFor({ state: 'hidden' });
    assert.equal(await page.evaluate(() => window.tunnel.enabled && !window.tunnel.running), true);
    await tunnelLabel.click();
    await startTunnel.click();
    const stopTunnel = tunnelDialog.getByRole('button', { name: 'Stop tunnel' });
    await stopTunnel.waitFor();
    assert.equal(await key.inputValue(), '');
    assert.match(await tunnelDialog.innerText(), /Tunnel is ready/);
    await stopTunnel.click();
    await startTunnel.waitFor();
    await closeTunnel();
    assert.equal(await tunnelToggle.isChecked(), false);
    await toggle.uncheck();
    assert.equal(await folder.isDisabled(), true);
    assert.equal(await tunnelToggle.isDisabled(), true);
    await page.evaluate(() => { window.toggleError = true; });
    await save.click();
    await page.waitForFunction(() => document.querySelector('#status-message')?.textContent.includes('Could not save Anyblob.'));
    assert.equal(await page.evaluate(() => window.enabled), true);
    assert.equal(await toggle.isChecked(), false);
    await page.evaluate(() => { window.toggleError = false; });
    await save.click();
    await save.waitFor({ state: 'hidden' });
    const original=await install.boundingBox();await install.click();const installing=panel.getByRole('button',{name:'Installing…',exact:true});assert.equal(await installing.isDisabled(),true);assert.equal((await installing.boundingBox()).width,original.width);
    await installing.evaluate(el=>el.click());assert.equal(await page.evaluate(()=>window.installCalls),1);await page.evaluate(()=>window.finishInstall());
    const complete=page.getByRole('dialog',{name:'Anyblob marketplace installed'});await complete.waitFor();assert.match(await complete.innerText(),/Plugins Directory/);await page.keyboard.press('Escape');await complete.waitFor({state:'hidden'});
    assert.equal((await panel.getByRole('button',{name:'Installed',exact:true}).boundingBox()).width,original.width);
    assert.equal(await page.locator('#codex-panel-0 .anyblob-version').innerText(),'v0.1.0');
    assert.equal(await panel.locator('.kanban-setting-title').first().evaluate(el=>getComputedStyle(el).fontSize),'14px');
    await page.locator('#preferences-nav').click();await page.evaluate(()=>window.updateAvailable=true);await page.locator('#codex-nav').click();
    const update=panel.getByRole('button',{name:'Update',exact:true});await update.waitFor();
    assert.equal(await page.locator('#codex-panel-0 .anyblob-version').innerText(),'v0.1.0 → v0.1.1');
    assert.equal(await panel.locator('.anyblob-version-available').evaluate(el=>getComputedStyle(el).color),'rgb(78, 204, 78)');
    await page.screenshot({path:path.join(fixture,'update-version-dark.png')});assert.equal((await update.boundingBox()).width,original.width);
    await page.evaluate(()=>{window.installedVersion='0.1.1';window.dispatchEvent(new Event('blobui-codex-integration-refresh'));});
    await page.waitForFunction(()=>document.querySelector('#codex-panel-0 .anyblob-version').textContent==='v0.1.1');
    assert.equal(await panel.getByRole('button',{name:'Update',exact:true}).isVisible(),true);
    await page.evaluate(()=>{window.installedVersion=null;window.dispatchEvent(new Event('blobui-codex-integration-refresh'));});
    await page.waitForFunction(()=>document.querySelector('#codex-panel-0 .anyblob-version').textContent.includes(' → '));
    await update.click();const updating=panel.getByRole('button',{name:'Updating…',exact:true});assert.equal(await updating.isDisabled(),true);
    await page.evaluate(()=>window.finishInstall());const updated=page.getByRole('dialog',{name:'Anyblob marketplace updated'});await updated.waitFor();assert.match(await updated.innerText(),/uninstall and reinstall/);await page.keyboard.press('Escape');await updated.waitFor({state:'hidden'});
    await update.waitFor();await page.evaluate(()=>window.updateAvailable=false);
    await page.evaluate(()=>{window.codexRefreshRequired=true;window.dispatchEvent(new Event('blobui-codex-integration-refresh'));});
    const refresh=panel.getByRole('button',{name:'Refresh in Codex',exact:true});await refresh.waitFor();
    assert.equal(await panel.locator('.anyblob-version').innerText(),'v0.1.0');
    const calls=await page.evaluate(()=>window.installCalls);await refresh.click();
    const refreshDialog=page.getByRole('dialog',{name:'Refresh Anyblob in Codex'});await refreshDialog.waitFor();
    assert.match(await refreshDialog.innerText(),/marketplace package is up to date/);assert.equal(await page.evaluate(()=>window.installCalls),calls);
    await page.keyboard.press('Escape');await refreshDialog.waitFor({state:'hidden'});
    await page.evaluate(()=>{window.codexRefreshRequired=false;window.dispatchEvent(new Event('blobui-codex-integration-refresh'));});
    await page.locator('#preferences-nav').click();await page.evaluate(()=>window.installed=false);await page.locator('#codex-nav').click();await install.waitFor();
    await page.evaluate(()=>window.installError=true);await install.click();await page.evaluate(()=>window.finishInstall());await install.waitFor();assert.match(await panel.locator('#anyblob-integration-status').innerText(),/Fixture install failed/);await page.evaluate(()=>window.installError=false);
    await page.locator('#codex-tab-0').focus();await page.keyboard.press('End');assert.equal(await page.locator('#codex-tab-2').getAttribute('aria-selected'),'true');await page.keyboard.press('Home');assert.equal(await page.locator('#codex-tab-0').getAttribute('aria-selected'),'true');
    await page.screenshot({path:path.join(fixture,'settings-original-dark.png')});
    if(await page.locator('#codex-nav').getAttribute('aria-expanded')!=='true')await page.locator('#codex-nav').click();await page.locator('#codex-section-links .v21-section-link').first().click();
    assert.equal(await panel.isVisible(),true);assert.equal(await page.locator('#codex-nav').innerText(),'Anyblob');
    await page.setViewportSize({width:800,height:750});await page.screenshot({path:path.join(fixture,'settings-refreshed-narrow-dark.png')});
    const box=await install.boundingBox();assert(box.x+box.width<=800);assert.equal(await page.evaluate(()=>document.documentElement.scrollWidth>innerWidth),false);
    await page.setViewportSize({width:1100,height:900});
    assert.equal(await page.evaluate(()=>Boolean(events['anyblob-approval']||events['anyblob-approval-closed'])),false);
    assert.deepEqual(errors,[]);
  }finally{await browser.close();}
});
