const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {loadSettings}=require('./frontend-fixture.cjs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');

test('Anyblob uses local project settings and opens the shared setup guide in a dark fixture',async()=>{
  const workspace=path.resolve(__dirname,'../..');
  const fixture=path.join(workspace,'.test-run/anyblob-ui');
  fs.mkdirSync(fixture,{recursive:true});
  process.env.TEMP=process.env.TMP=fixture;
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:900},colorScheme:'dark'}),errors=[];
    page.on('pageerror',error=>errors.push(error.message));
    await page.addInitScript(()=>{
      window.calls=[];window.enabled=false;window.localProjectsFolder='';window.folderSelection=null;
      window.__TAURI__={event:{listen:async()=>()=>{}},core:{invoke:async(name,args)=>{
        calls.push(name);
        if(name==='get_app_state')return{ready:true,onboardingComplete:true,theme:'dark',settings:{}};
        if(name==='get_updater_state')return{automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return'available';
        if(name==='anyblob_status')return{enabled:window.enabled,localProjectsFolder:window.localProjectsFolder};
        if(name==='configure_anyblob'){window.enabled=args.enabled;return{enabled:args.enabled};}
        if(name==='select_anyblob_projects_folder')return window.folderSelection;
        if(name==='set_anyblob_projects_folder'){window.localProjectsFolder=args.folder;return{localProjectsFolder:args.folder};}
        if(name==='save_settings')return{};
        return null;
      }}};
    });
    await loadSettings(page);
    await page.waitForFunction(()=>window.appReady);
    await page.locator('#codex-nav').click();
    const panel=page.locator('#codex-panel-0');
    const toggle=panel.getByRole('switch',{name:'Enable Anyblob'});
    const folder=panel.getByRole('button',{name:'Local Projects Folder',exact:true});
    assert.equal(await toggle.isChecked(),false);
    assert.equal(await folder.isDisabled(),true);
    assert.equal(await panel.getByRole('button',{name:'Install',exact:true}).count(),0);
    assert.equal(await panel.getByRole('switch',{name:'Secure MCP Tunnel'}).count(),0);
    assert.equal(await panel.getByRole('heading',{name:'Remote Codex',exact:true}).count(),0);

    await toggle.check();
    assert.equal(await page.evaluate(()=>window.enabled),false,'Toggle changes stay in the settings draft before Save.');
    await page.evaluate(()=>{window.folderSelection='C:/Fixture Projects';});
    await folder.click();
    assert.equal(await folder.inputValue(),'C:/Fixture Projects');
    await page.locator('#save-button').click();
    await page.locator('#save-button').waitFor({state:'hidden'});
    assert.equal(await page.evaluate(()=>window.enabled),true);
    assert.equal(await page.evaluate(()=>window.localProjectsFolder),'C:/Fixture Projects');

    await panel.getByRole('button',{name:'Anyblob help'}).click();
    const guide=page.getByRole('dialog',{name:'Anyblob setup guide'});
    await guide.waitFor();
    await guide.getByRole('heading',{name:'1. Start Anytype Headless'}).waitFor();
    assert.match(await guide.innerText(),/blob-ui api login/);
    assert.match(await guide.innerText(),/codex plugin marketplace add PainterFlow\/Anytype-BlobUI/);
    await page.screenshot({path:path.join(fixture,'anyblob-guide-dark.png'),animations:'disabled'});
    await page.keyboard.press('Escape');
    await guide.waitFor({state:'hidden'});
    assert.equal(await page.evaluate(()=>calls.some(name=>/tunnel|marketplace/i.test(name))),false);
    assert.deepEqual(errors,[]);
  }finally{await browser.close();}
});