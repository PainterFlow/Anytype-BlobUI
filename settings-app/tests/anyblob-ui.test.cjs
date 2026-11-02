const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
test('Anyblob setup has no approval listeners in a dark isolated fixture',async()=>{
  const workspace=path.resolve(__dirname,'../..'),fixture=path.join(workspace,'.test-run/anyblob-ui');fs.mkdirSync(fixture,{recursive:true});process.env.TEMP=process.env.TMP=fixture;
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:900},colorScheme:'dark'}),errors=[];page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(()=>{
      window.events={};window.decisions=[];window.enabled=false;window.installed=false;window.installCalls=0;
      window.__TAURI__={event:{listen:async(name,callback)=>{window.events[name]=callback;return()=>{};}},core:{invoke:async(name,args)=>{
        if(name==='get_app_state')return{ready:true,onboardingComplete:true,theme:'dark',settings:{}};
        if(name==='get_updater_state')return{automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return'available';
        if(name==='anyblob_status')return{enabled:window.enabled};
        if(name==='configure_anyblob'){if(window.toggleError)throw Error('Could not save Anyblob.');window.enabled=args.enabled;return{enabled:args.enabled};}
        if(name==='anyblob_installation_status')return{installed:window.installed,updateAvailable:!!window.updateAvailable,bundledVersion:'0.1.1',installedVersion:window.installed?'0.1.0':null};
        if(name==='install_anyblob_marketplace'){window.installCalls++;await new Promise(resolve=>window.finishInstall=resolve);if(window.installError)throw Error('Fixture install failed');window.installed=true;return null;}
        return null;
      }}};
    });
    await page.goto(pathToFileURL(path.join(workspace,'settings-app/ui/index.html')).href);await page.waitForFunction(()=>window.appReady);await page.locator('#codex-nav').click();
    const panel=page.locator('#codex-panel-0'),toggle=panel.locator('input[type=checkbox]').first(),install=panel.getByRole('button',{name:'Install',exact:true});
    assert.equal(await page.locator('#codex-nav .nav-label').count() ? await page.locator('#codex-nav .nav-label').innerText() : await page.locator('#codex-nav').innerText(),'Anyblob');
    assert.deepEqual(await panel.locator('h2').allTextContents(),['General','Object Mentions']);assert.equal(await panel.locator('.settings-list > .settings-row').count(),5);
    assert.equal(await panel.getByRole('textbox').count(),0);assert.doesNotMatch(await panel.innerText(),/token|Build-Codex|environment|Copy/);
    assert.equal(await toggle.isChecked(),false);await toggle.check();await page.waitForFunction(()=>window.enabled);await toggle.uncheck();await page.waitForFunction(()=>!window.enabled);
    await page.evaluate(()=>window.toggleError=true);await toggle.click();await page.waitForFunction(()=>document.querySelector('#codex-panel-0 [role=status]').textContent.includes('Could not save'));assert.equal(await toggle.isChecked(),false);await page.evaluate(()=>window.toggleError=false);
    const original=await install.boundingBox();await install.click();const installing=panel.getByRole('button',{name:'Installing…',exact:true});assert.equal(await installing.isDisabled(),true);assert.equal((await installing.boundingBox()).width,original.width);
    await installing.evaluate(el=>el.click());assert.equal(await page.evaluate(()=>window.installCalls),1);await page.evaluate(()=>window.finishInstall());
    const complete=page.getByRole('dialog',{name:'Anyblob marketplace installed'});await complete.waitFor();assert.match(await complete.innerText(),/Plugins Directory/);await page.keyboard.press('Escape');await complete.waitFor({state:'hidden'});
    assert.equal((await panel.getByRole('button',{name:'Installed',exact:true}).boundingBox()).width,original.width);
    assert.equal(await page.locator('#codex-panel-0 .anyblob-version').innerText(),'v0.1.0');
    await page.locator('#preferences-nav').click();await page.evaluate(()=>window.updateAvailable=true);await page.locator('#codex-nav').click();
    const update=panel.getByRole('button',{name:'Update',exact:true});await update.waitFor();
    assert.equal(await page.locator('#codex-panel-0 .anyblob-version').innerText(),'v0.1.0 → v0.1.1');
    assert.equal(await panel.locator('.anyblob-version-available').evaluate(el=>getComputedStyle(el).color),'rgb(78, 204, 78)');
    await page.screenshot({path:path.join(fixture,'update-version-dark.png')});assert.equal((await update.boundingBox()).width,original.width);
    await update.click();const updating=panel.getByRole('button',{name:'Updating…',exact:true});assert.equal(await updating.isDisabled(),true);
    await page.evaluate(()=>window.finishInstall());const updated=page.getByRole('dialog',{name:'Anyblob marketplace updated'});await updated.waitFor();assert.match(await updated.innerText(),/uninstall and reinstall/);await page.keyboard.press('Escape');await updated.waitFor({state:'hidden'});
    await update.waitFor();await page.evaluate(()=>window.updateAvailable=false);
    await page.locator('#preferences-nav').click();await page.evaluate(()=>window.installed=false);await page.locator('#codex-nav').click();await install.waitFor();
    await page.evaluate(()=>window.installError=true);await install.click();await page.evaluate(()=>window.finishInstall());await install.waitFor();assert.match(await panel.locator('[role=status]').first().innerText(),/Fixture install failed/);await page.evaluate(()=>window.installError=false);
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
