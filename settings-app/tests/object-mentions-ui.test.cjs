const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const { loadSettings } = require('./frontend-fixture.cjs');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');

test('unified object mention type filter preserves drafts, icons, sections, and derived spaces',async()=>{
  const workspace=path.resolve(__dirname,'../..'),fixture=path.join(workspace,'.test-run/object-mentions-ui');
  fs.mkdirSync(fixture,{recursive:true});process.env.TEMP=process.env.TMP=fixture;
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:900},colorScheme:'dark'}),errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(()=>{
      window.saved=null;window.typeRequests=[];
      window.fixtureSpaces=[{id:'a',name:'Work',icon:{emoji:'💼'}},{id:'b',name:'Personal',icon:{emoji:'🏠'}},{id:'noperm',name:'No Access',icon:{emoji:'🚫'}}];
      const permitted=Object.fromEntries(['a','b'].map(id=>[id,{access:'read',filesEnabled:false,approval:'fullAccess'}]));
      window.__TAURI__={core:{invoke:async(name,args)=>{
        if(name==='get_app_state')return{ready:true,onboardingComplete:true,theme:'dark',settings:{codexIntegration:{spaces:permitted}}};
        if(name==='get_updater_state')return{automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return'available';
        if(name==='anyblob_status')return{enabled:false};
        if(name==='discover_codex_spaces')return window.fixtureSpaces;
        if(name==='discover_anyblob_types'){
          window.typeRequests.push(args.spaceIds);
          return{types:args.spaceIds.map(spaceId=>({spaceId,id:'task',name:'Task',space:spaceId==='a'?'Work':'Personal',icon:{emoji:'✅'}})),errors:[]};
        }
        if(name==='save_settings'){window.saved=structuredClone(args.settings);return{};}
        return null;
      }}};
    });
    await loadSettings(page);
    await page.waitForFunction(()=>window.appReady);
    await page.locator('#codex-nav').click();
    const input=page.getByRole('combobox',{name:'Filtered Types',exact:true});
    const menu=page.getByRole('listbox',{name:'Filtered Types',exact:true});
    const incomplete=page.getByRole('switch',{name:'Filter Incomplete Tasks',exact:true});
    await page.waitForFunction(()=>window.typeRequests.length>0);
    assert.equal(await input.inputValue(),'All');
    assert.equal(await page.evaluate(()=>window.typeRequests.every(ids=>ids.includes('a')&&ids.includes('b')&&!ids.includes('noperm'))),true,'Type discovery covers every readable space and no space without read permission');

    await input.click();
    await menu.getByRole('option',{name:'All',exact:true}).waitFor();
    assert.equal(await menu.locator('.selector-menu-section',{hasText:'No Access'}).count(),0);
    assert.equal(await menu.locator('.selector-menu-section',{hasText:'Work'}).count(),1);
    assert.equal(await menu.locator('.selector-menu-section',{hasText:'Personal'}).count(),1);
    assert.equal(await menu.locator('.selector-menu-section .selector-option-icon').count(),0,'Space category headers do not use icons');
    assert.ok(await menu.locator('.selector-option-icon').count()>0,'Type options render their cached icons');
    await menu.getByRole('option',{name:'All',exact:true}).click();
    await page.keyboard.press('Escape');
    assert.equal(await input.inputValue(),'None');

    await input.click();
    await menu.getByRole('option',{name:'Task',exact:true}).first().click();
    await menu.getByRole('option',{name:'Task',exact:true}).last().click();
    await page.keyboard.press('Escape');
    assert.equal(await input.inputValue(),'Task, Task');
    assert.equal(await page.locator('.mention-picker .advanced-selector-rich-display .selector-option-icon').count(),2,'Selected type icons appear inline in the closed picker');
    await incomplete.check();
    await page.locator('#save-button').click();
    await page.waitForFunction(()=>window.saved?.codexIntegration?.objectMentions?.types?.items?.length===2);
    const mentions=await page.evaluate(()=>window.saved.codexIntegration.objectMentions);
    assert.equal(mentions.filterIncompleteTasks,true);
    assert.deepEqual(mentions.spaces,{all:false,items:[{id:'b',name:'Personal'},{id:'a',name:'Work'}]},'Selecting types derives the spaces filter');
    assert.deepEqual(mentions.types.items.map(row=>[row.spaceId,row.id]),[['b','task'],['a','task']]);

    await incomplete.uncheck();
    await page.evaluate(()=>window.BlobUI.discardChanges());
    assert.equal(await incomplete.isChecked(),true);
    assert.equal(await input.inputValue(),'Task, Task');

    await input.click();
    await input.fill('personal');
    assert.equal(await menu.getByRole('option',{name:'Task',exact:true}).count(),1,'Search narrows options by their space category');
    assert.equal(await menu.locator('.selector-menu-section',{hasText:'Personal'}).count(),1);
    await page.keyboard.press('Escape');
    assert.equal(await input.evaluate(field=>getComputedStyle(field).textOverflow),'ellipsis');

    await page.setViewportSize({width:620,height:650});
    await input.click();
    await page.waitForFunction(()=>document.querySelector('[role=listbox][aria-label="Filtered Types"]')?.dataset.state==='open');
    const position=await menu.evaluate(element=>{
      const rect=element.getBoundingClientRect();
      return{left:rect.left,right:rect.right,top:rect.top,bottom:rect.bottom,width:rect.width};
    });
    assert.ok(position.width>=300,'The shared menu is larger than the legacy picker');
    assert.ok(position.left>=0&&position.right<=620,'The menu stays inside the viewport horizontally');
    assert.ok(position.top>=0&&position.bottom<=650,'The menu stays inside the viewport vertically');
    await page.keyboard.press('Escape');
    await page.screenshot({path:path.join(fixture,'filtered-types-dark.png'),fullPage:true});
    assert.deepEqual(errors,[]);
  }finally{await browser.close();}
});
