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
      window.events={};window.decisions=[];window.enabled=false;window.remoteAccess=false;window.installed=false;window.installCalls=0;window.clipboardWrites=[];
      Object.defineProperty(navigator,'clipboard',{configurable:true,value:{writeText:async value=>{if(window.clipboardFails)throw Error('Clipboard unavailable.');window.clipboardWrites.push(value);}}});
      window.__TAURI__={event:{listen:async(name,callback)=>{window.events[name]=callback;return()=>{};}},core:{invoke:async(name,args)=>{
        if(name==='get_app_state')return{ready:true,onboardingComplete:true,theme:'dark',settings:{}};
        if(name==='get_updater_state')return{automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return'available';
        if(name==='anyblob_status')return{enabled:window.enabled,remoteAccess:window.remoteAccess};
        if(name==='configure_anyblob'){if(window.toggleError)throw Error('Could not save Anyblob.');window.enabled=args.enabled;if(!args.enabled)window.remoteAccess=false;return{enabled:args.enabled};}
        if(name==='configure_anyblob_remote'){if(window.remoteToggleError)throw Error('Could not save remote access.');window.remoteAccess=args.enabled;return{enabled:args.enabled,token:args.enabled?'fixture-remote-key':''};}
        if(name==='start_cloudflare_tunnel'){window.tunnelStarts=(window.tunnelStarts||0)+1;return{url:'https://sample.trycloudflare.com'};}
        if(name==='stop_cloudflare_tunnel'){window.tunnelStops=(window.tunnelStops||0)+1;return null;}
        if(name==='anyblob_installation_status')return{installed:window.installed,updateAvailable:!!window.updateAvailable,codexRefreshRequired:!!window.codexRefreshRequired,bundledVersion:'0.1.1',installedVersion:window.installed?(window.installedVersion||'0.1.0'):null};
        if(name==='install_anyblob_marketplace'){window.installCalls++;await new Promise(resolve=>window.finishInstall=resolve);if(window.installError)throw Error('Fixture install failed');window.installed=true;return null;}
        return null;
      }}};
    });
    await loadSettings(page);await page.waitForFunction(()=>window.appReady);await page.locator('#codex-nav').click();
    const panel=page.locator('#codex-panel-0'),toggle=panel.locator('input[type=checkbox]').first(),install=panel.getByRole('button',{name:'Install',exact:true});
    assert.equal(await page.locator('#codex-nav .nav-label').count() ? await page.locator('#codex-nav .nav-label').innerText() : await page.locator('#codex-nav').innerText(),'Anyblob');
    assert.deepEqual(await panel.locator('h2').allTextContents(),['General','Object Mentions']);assert.equal(await panel.locator('.settings-list > .settings-row').count(),6);
    assert.equal(await panel.getByRole('heading',{name:'Remote Codex',exact:true}).count(),0);
    const integrationGroups=panel.locator('.anyblob-integration-settings');assert.equal(await integrationGroups.count(),2);
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
    assert.equal(await panel.getByRole('textbox').count(),0);assert.doesNotMatch(await panel.innerText(),/token|Build-Codex|environment|Copy/);
    const remoteToggle=panel.locator('#enable-anyblob-remote'),remoteLabel=panel.getByRole('button',{name:'Allow Cloudflare Tunnel remote access',exact:true});
    assert.equal(await toggle.isChecked(),false);assert.equal(await remoteToggle.isDisabled(),true);assert.equal(await remoteLabel.isDisabled(),true);
    await remoteLabel.evaluate(el=>el.click());assert.equal(await page.locator('#anyblob-remote-access-dialog').isVisible(),false);
    const remoteHelp=panel.getByRole('button',{name:'Allow Cloudflare Tunnel remote access information'});await remoteHelp.focus();
    const remoteTooltip=page.locator(`#${await remoteHelp.getAttribute('aria-describedby')}`);await remoteTooltip.waitFor({state:'visible'});
    assert.equal(await remoteTooltip.evaluate(el=>el.parentElement===document.body),true);
    assert.match(await remoteTooltip.innerText(),/cloud rows appear under the remote PC name/);
    assert.match(await remoteTooltip.innerText(),/Missing or invalid membership is denied when scopes exist/);
    await page.screenshot({path:path.join(fixture,'remote-access-help-tooltip-dark.png')});
    await toggle.check();await page.waitForFunction(()=>window.enabled);assert.equal(await remoteToggle.isDisabled(),false);assert.equal(await remoteLabel.isDisabled(),true);
    await remoteToggle.check();await page.waitForFunction(()=>window.remoteAccess);
    const remoteDialog=page.getByRole('dialog',{name:'Remote access'});await remoteDialog.waitFor();
    assert.equal(await remoteToggle.isChecked(),true);assert.equal(await remoteDialog.locator('.settings-list').count(),0);assert.equal(await remoteDialog.locator('.anyblob-remote-dialog-group').count(),2);
    assert.equal(await remoteDialog.locator('.anyblob-remote-dialog-group').first().evaluate(el=>getComputedStyle(el).borderTopWidth),'0px');
    assert.equal(await remoteDialog.getByRole('textbox',{name:'Key',exact:true}).getAttribute('type'),'password');
    assert.match(await remoteDialog.innerText(),/Quick Tunnel URLs are temporary/);assert.match(await remoteDialog.innerText(),/ANYBLOB_REMOTE_ADDRESS/);
    assert.equal(await remoteDialog.getByRole('button',{name:'Copy key'}).isDisabled(),false);
    await remoteDialog.getByRole('button',{name:'Copy key'}).click();await remoteDialog.getByRole('button',{name:'Copied'}).waitFor();
    assert.equal(await remoteDialog.getByRole('status').innerText(),'Remote access key copied.');assert.deepEqual(await page.evaluate(()=>window.clipboardWrites),['fixture-remote-key']);
    await remoteDialog.getByRole('button',{name:'Copy key'}).waitFor();
    await remoteDialog.getByRole('button',{name:'Show remote access key'}).click();assert.equal(await remoteDialog.getByRole('textbox',{name:'Key',exact:true}).getAttribute('type'),'text');
    await remoteDialog.getByRole('button',{name:'Hide remote access key'}).click();assert.equal(await remoteDialog.getByRole('textbox',{name:'Key',exact:true}).getAttribute('type'),'password');
    await remoteDialog.getByRole('button',{name:'Start tunnel'}).click();await remoteDialog.getByRole('button',{name:'Stop tunnel'}).waitFor();
    const tunnelInput=remoteDialog.getByRole('textbox',{name:'Cloudflare Tunnel URL'}),tunnelField=remoteDialog.locator('.copy-text-field');
    assert.equal(await tunnelInput.inputValue(),'https://sample.trycloudflare.com');assert.equal(await tunnelInput.evaluate(el=>el.readOnly),true);
    assert.equal(await tunnelField.evaluate(el=>getComputedStyle(el).borderRadius),'18px');assert.equal(await tunnelInput.evaluate(el=>getComputedStyle(el).minHeight),'48px');
    const copyTunnel=remoteDialog.getByRole('button',{name:'Copy tunnel URL'});
    await tunnelInput.focus();await page.keyboard.press('Tab');assert.equal(await copyTunnel.evaluate(el=>el===document.activeElement),true);
    await page.keyboard.press('Space');await remoteDialog.getByRole('button',{name:'Tunnel URL copied'}).waitFor();
    assert.equal(await remoteDialog.getByRole('status').innerText(),'Tunnel URL copied.');
    const copiedTunnel=remoteDialog.getByRole('button',{name:'Tunnel URL copied'});
    assert.equal(await copiedTunnel.evaluate(el=>el.parentElement.dataset.copied),'true');
    await copiedTunnel.hover();
    const copiedHover=await copiedTunnel.evaluate(el=>{
      const buttonStyle=getComputedStyle(el),probe=document.createElement('div');
      probe.style.backgroundColor='color-mix(in srgb, var(--color-green) 16%, transparent)';document.body.append(probe);
      const expectedBackground=getComputedStyle(probe).backgroundColor;probe.remove();
      return{color:buttonStyle.color,background:buttonStyle.backgroundColor,expectedBackground};
    });
    assert.equal(copiedHover.color,'rgb(78, 204, 78)');assert.equal(copiedHover.background,copiedHover.expectedBackground);
    await page.evaluate(()=>window.clipboardFails=true);await remoteDialog.getByRole('button',{name:'Tunnel URL copied'}).click();await remoteDialog.getByRole('button',{name:'Copy tunnel URL'}).waitFor();
    assert.match(await remoteDialog.getByRole('status').innerText(),/Could not copy the tunnel URL/);
    await page.evaluate(()=>window.clipboardFails=false);await tunnelInput.focus();await page.keyboard.press('Tab');await page.keyboard.press('Space');
    await remoteDialog.getByRole('button',{name:'Tunnel URL copied'}).waitFor();
    assert.equal(await remoteDialog.getByRole('status').innerText(),'Tunnel URL copied.');
    assert.deepEqual(await page.evaluate(()=>window.clipboardWrites),['fixture-remote-key','https://sample.trycloudflare.com','https://sample.trycloudflare.com']);
    await remoteDialog.screenshot({path:path.join(fixture,'remote-access-dialog-dark.png')});
    await page.setViewportSize({width:800,height:750});await remoteDialog.screenshot({path:path.join(fixture,'remote-access-dialog-narrow-dark.png')});
    await page.setViewportSize({width:1100,height:900});
    await remoteDialog.getByRole('button',{name:'Copy tunnel URL'}).waitFor();assert.equal(await remoteDialog.getByRole('status').innerText(),'');
    await remoteDialog.getByRole('button',{name:'Stop tunnel'}).click();await remoteDialog.getByRole('button',{name:'Start tunnel'}).waitFor();
    await remoteDialog.getByRole('button',{name:'Close remote access'}).click();await remoteDialog.waitFor({state:'hidden'});
    assert.equal(await remoteToggle.isChecked(),true);assert.equal(await remoteLabel.isDisabled(),false);
    await remoteLabel.click();await remoteDialog.waitFor();assert.equal(await remoteToggle.isChecked(),true);
    await remoteDialog.getByRole('button',{name:'Close remote access'}).click();await remoteDialog.waitFor({state:'hidden'});
    await remoteToggle.uncheck();await page.waitForFunction(()=>!window.remoteAccess);assert.equal(await remoteDialog.isVisible(),false);assert.equal(await remoteLabel.isDisabled(),true);
    await toggle.uncheck();await page.waitForFunction(()=>!window.enabled);assert.equal(await remoteToggle.isDisabled(),true);assert.equal(await remoteLabel.isDisabled(),true);
    await page.evaluate(()=>window.toggleError=true);await toggle.click();await page.waitForFunction(()=>document.querySelector('#codex-panel-0 [role=status]').textContent.includes('Could not save'));assert.equal(await toggle.isChecked(),false);await page.evaluate(()=>window.toggleError=false);
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
