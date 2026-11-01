const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
test('Anyblob setup and request-bound approvals in a dark isolated fixture',async()=>{
  const workspace=path.resolve(__dirname,'../..'),fixture=path.join(workspace,'.test-run/anyblob-ui');fs.mkdirSync(fixture,{recursive:true});process.env.TEMP=process.env.TMP=fixture;
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:900},colorScheme:'dark'}),errors=[];page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(()=>{
      window.events={};window.decisions=[];window.enabled=false;
      window.__TAURI__={event:{listen:async(name,callback)=>{window.events[name]=callback;return()=>{};}},core:{invoke:async(name,args)=>{
        if(name==='get_app_state')return{ready:true,onboardingComplete:true,theme:'dark',settings:{}};
        if(name==='get_updater_state')return{automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return'available';
        if(name==='anyblob_status')return{enabled:window.enabled};
        if(name==='configure_anyblob'){window.enabled=args.enabled;return{enabled:args.enabled,token:args.enabled?'fixture-token-shown-once':null};}
        if(name==='approve_anyblob'){window.decisions.push(args);return null;}
        return null;
      }}};
    });
    await page.goto(pathToFileURL(path.join(workspace,'settings-app/ui/index.html')).href);await page.waitForFunction(()=>window.appReady);await page.locator('#codex-nav').click();
    const toggle=page.locator('#codex-panel-0 input[type=checkbox]'),token=page.getByRole('textbox',{name:'New Anyblob bridge token'});
    assert.equal(await toggle.isChecked(),false);await toggle.check();await token.waitFor();assert.equal(await token.inputValue(),'fixture-token-shown-once');
    await page.getByRole('button',{name:'Hide token'}).click();assert.equal(await token.isVisible(),false);
    await page.getByRole('button',{name:'Regenerate token'}).click();await token.waitFor();await toggle.uncheck();assert.equal(await token.isVisible(),false);
    const send=async(id,operation='search_objects')=>page.evaluate(({id,operation})=>window.events['anyblob-approval']({payload:{id,request:{operation,space_id:'space-exact',object_id:'object-exact',query:'<script>not executable</script>',limit:20,offset:0}}}),{id,operation});
    await send('request-one');const dialog=page.getByRole('dialog',{name:'Anyblob read approval'});await dialog.waitFor();assert.match(await dialog.innerText(),/space-exact/);assert.match(await dialog.innerText(),/<script>not executable<\/script>/);assert.equal(await dialog.locator('script').count(),0);
    await dialog.evaluate(el=>Promise.all(el.closest('.confirm-backdrop').getAnimations({subtree:true}).map(animation=>animation.finished)));
    await page.screenshot({path:path.join(fixture,'approval-dark.png')});
    await page.getByRole('button',{name:'Allow once'}).click();await page.waitForFunction(()=>decisions.length===1);assert.deepEqual(await page.evaluate(()=>decisions[0]),{id:'request-one',allow:true});
    await dialog.waitFor({state:'hidden'});await send('request-two','read_object');await dialog.waitFor();assert.match(await dialog.innerText(),/object-exact/);await page.keyboard.press('Escape');await page.waitForFunction(()=>decisions.length===2);assert.deepEqual(await page.evaluate(()=>decisions[1]),{id:'request-two',allow:false});
    await dialog.waitFor({state:'hidden'});await send('request-three');await dialog.waitFor();await page.evaluate(()=>events['anyblob-approval-closed']({payload:{id:'request-three'}}));await dialog.waitFor({state:'hidden'});assert.equal(await page.evaluate(()=>decisions.length),2);
    await send('request-four');await dialog.waitFor();
    await page.evaluate(()=>{events['anyblob-approval-closed']({payload:{id:'request-four'}});events['anyblob-approval']({payload:{id:'request-five',request:{operation:'read_object',space_id:'space-exact',object_id:'next-object'}}});});
    await page.evaluate(()=>new Promise(resolve=>setTimeout(resolve,240)));
    assert.equal(await page.locator('#codex-page').evaluate(el=>!!el.closest('[inert]')),true);
    assert.equal(await dialog.count(),1);assert.match(await dialog.innerText(),/next-object/);
    await page.getByRole('button',{name:'Deny',exact:true}).click();
    assert.deepEqual(errors,[]);
  }finally{await browser.close();}
});
