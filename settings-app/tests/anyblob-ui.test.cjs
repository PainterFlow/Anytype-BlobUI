const test=require('node:test');
const assert=require('node:assert/strict');
const fs=require('node:fs');
const path=require('node:path');
const {loadSettings}=require('./frontend-fixture.cjs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');

test('Anyblob settings omit local project controls and open the setup guide in a dark fixture',async()=>{
  const workspace=path.resolve(__dirname,'../..');
  const fixture=path.join(workspace,'.test-run/anyblob-ui');
  fs.mkdirSync(fixture,{recursive:true});
  process.env.TEMP=process.env.TMP=fixture;
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:900},colorScheme:'dark'}),errors=[];
    page.on('pageerror',error=>errors.push(error.message));
    await page.addInitScript(()=>{
      window.calls=[];window.enabled=false;window.agentsGuide=false;
      window.__TAURI__={event:{listen:async()=>()=>{}},core:{invoke:async(name,args)=>{
        calls.push(name);
        if(name==='get_app_state')return{ready:true,onboardingComplete:true,theme:'dark',settings:{}};
        if(name==='get_updater_state')return{automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return'available';
        if(name==='anyblob_status')return{enabled:window.enabled,agentsGuide:window.agentsGuide};
        if(name==='configure_anyblob'){window.enabled=args.enabled;return{enabled:args.enabled};}
        if(name==='configure_anyblob_agents_guide'){window.agentsGuide=args.agentsGuide;return{agentsGuide:args.agentsGuide};}
        if(name==='save_settings')return{};
        return null;
      }}};
    });
    await loadSettings(page);
    await page.waitForFunction(()=>window.appReady);
    await page.locator('#codex-nav').click();
    const panel=page.locator('#codex-panel-0');
    const toggle=panel.getByRole('switch',{name:'Enable Anyblob'});
    assert.equal(await toggle.isChecked(),false);
    assert.equal(await page.getByRole('tab',{name:'Projects'}).count(),0);
    assert.equal(await panel.getByRole('button',{name:'Local Projects Folder',exact:true}).count(),0);
    assert.equal(await panel.getByRole('button',{name:'Install',exact:true}).count(),0);
    assert.equal(await panel.getByRole('switch',{name:'Secure MCP Tunnel'}).count(),0);
    assert.equal(await panel.getByRole('heading',{name:'Remote Codex',exact:true}).count(),0);

    assert.equal(await panel.getByRole('heading',{name:'Advanced Settings',exact:true}).count(),1);
    const headings=await panel.locator('.section-heading h2').allInnerTexts();
    assert.ok(headings.indexOf('Advanced Settings')>headings.indexOf('Object Mentions'),'Advanced Settings renders below Object Mentions.');
    const guideToggle=panel.getByRole('switch',{name:/Enable AGENTS\.md page/});
    assert.equal(await guideToggle.isChecked(),false);
    assert.equal(await guideToggle.isDisabled(),true,'AGENTS.md page stays off while Anyblob is disabled.');
    const guideHelp=panel.getByRole('button',{name:'Enable AGENTS.md page help'});
    assert.equal(await guideHelp.count(),1);
    await guideHelp.hover();
    assert.equal(await page.locator('[role="tooltip"][data-open="true"]').textContent(),'For each space, you can create an object of any type(unless it is a collection/query) called "agents guide" the agent will always read all objects with that name before using Anyblob and will follow that objects guidelines like AGENTS.md.');

    await toggle.check();
    assert.equal(await guideToggle.isDisabled(),false,'AGENTS.md page unlocks while Anyblob is enabled.');
    await guideToggle.check();
    assert.equal(await page.evaluate(()=>window.enabled),false,'Toggle changes stay in the settings draft before Save.');
    await page.locator('#save-button').click();
    await page.locator('#save-button').waitFor({state:'hidden'});
    assert.equal(await page.evaluate(()=>window.enabled),true);
    assert.equal(await page.evaluate(()=>window.agentsGuide),true);

    const helpButton=panel.getByRole('button',{name:'Anyblob help'});
    assert.equal(await helpButton.getAttribute('aria-describedby'),null);
    assert.equal(await helpButton.getAttribute('aria-haspopup'),'dialog');
    await helpButton.click();
    const guide=page.getByRole('dialog',{name:'Anyblob setup guide'});
    await guide.waitFor();
    await guide.getByRole('heading',{name:/Part 1: Installing Anytype/}).waitFor();
    await guide.locator('.markdown-video video').waitFor();
    assert.match(await guide.locator('.markdown-video video').getAttribute('src'),/^blob:/,'The configured guide video must load from the media manifest.');
    assert.match(await guide.innerText(),/blob-ui api login/);
    assert.match(await guide.innerText(),/codex plugin marketplace add https:\/\/github\.com\/PainterFlow\/Anytype-BlobUI/);
    await page.screenshot({path:path.join(fixture,'anyblob-guide-dark.png'),animations:'disabled'});
    await page.keyboard.press('Escape');
    await guide.waitFor({state:'hidden'});
    assert.equal(await page.evaluate(()=>calls.some(name=>/tunnel|marketplace/i.test(name))),false);
    assert.deepEqual(errors,[]);
  }finally{await browser.close();}
});
