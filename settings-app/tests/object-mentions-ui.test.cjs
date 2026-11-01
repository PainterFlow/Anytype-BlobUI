const test=require('node:test'),assert=require('node:assert/strict'),fs=require('node:fs'),path=require('node:path');
const {pathToFileURL}=require('node:url');
const {chromium}=require(process.env.PLAYWRIGHT_MODULE||'playwright');
test('object mention filters stay editable and preserve saved drafts in dark mode',async()=>{
  const workspace=path.resolve(__dirname,'../..'),fixture=path.join(workspace,'.test-run/object-mentions-ui');
  fs.mkdirSync(fixture,{recursive:true});process.env.TEMP=process.env.TMP=fixture;
  const browser=await chromium.launch({executablePath:'C:/Program Files/Google/Chrome/Application/chrome.exe',headless:true,ignoreDefaultArgs:['--hide-scrollbars']});
  try {
    const page=await browser.newPage({viewport:{width:1100,height:1000},colorScheme:'dark'}),errors=[];
    page.on('pageerror',e=>errors.push(e.message));
    await page.addInitScript(()=>{
      window.saved=null;window.discoveryPending=true;window.spaceError=false;window.typeError=false;
      window.fixtureSpaces=[{id:'a',name:'Work'},{id:'b',name:'Personal'}];window.typeRequests=[];
      window.__TAURI__={core:{invoke:async(name,args)=>{
        if(name==='get_app_state')return{ready:true,onboardingComplete:true,theme:'dark',settings:{}};
        if(name==='get_updater_state')return{automaticChecksEnabled:false};
        if(name==='get_calendar_api_availability')return'available';
        if(name==='anyblob_status')return{enabled:false};
        if(name==='anyblob_installation_status')return{installed:false};
        if(name==='discover_codex_spaces'){
          if(window.discoveryPending)await new Promise(resolve=>window.releaseSpaces=resolve);
          if(window.spaceError)throw Error('Fixture space discovery failed');return window.fixtureSpaces;
        }
        if(name==='discover_anyblob_types'){
          window.typeRequests.push(args.spaceIds);
          if(window.holdTypes)await new Promise(resolve=>window.releaseTypes=resolve);
          if(window.typeError)throw Error('Fixture type discovery failed');
          return{types:window.omitTypes?[]:args.spaceIds.map(spaceId=>({spaceId,id:'task',name:'Task',space:spaceId==='a'?'Work':'Personal'})),errors:[]};
        }
        if(name==='save_settings'){window.saved=structuredClone(args.settings);return{};}
        return null;
      }}};
    });
    await page.goto(pathToFileURL(path.join(workspace,'settings-app/ui/index.html')).href);
    await page.waitForFunction(()=>window.appReady);await page.locator('#codex-nav').click();
    const spaces=page.getByRole('button',{name:'Filtered spaces',exact:true}),types=page.getByRole('button',{name:'Filtered object types',exact:true});
    const spaceMenu=page.getByRole('listbox',{name:'Filtered spaces',exact:true}),typeMenu=page.getByRole('listbox',{name:'Filtered object types',exact:true});
    const incomplete=page.getByRole('switch',{name:'Filter Incomplete Tasks',exact:true});
    assert.equal(await incomplete.isChecked(),false);
    assert.equal(await spaces.isEnabled(),true);assert.equal(await types.isEnabled(),true);
    assert.equal(await spaces.innerText(),'All');assert.equal(await types.innerText(),'All');
    assert.equal(await page.evaluate(()=>{
      const heading=document.querySelector('#object-mentions-section h2').getBoundingClientRect();
      const general=document.querySelector('#codex-panel-0 > .settings-list').getBoundingClientRect();
      return heading.top-general.bottom>=32;
    }),true);
    await types.focus();await page.keyboard.press('ArrowDown');await typeMenu.getByRole('option',{name:'All',exact:true}).waitFor();
    await page.keyboard.press('Enter');await page.keyboard.press('Escape');assert.equal(await types.innerText(),'None');
    // An empty type selection cannot lock either picker while spaces are still loading.
    assert.equal(await spaces.isEnabled(),true);assert.equal(await types.isEnabled(),true);
    await page.evaluate(()=>{window.holdTypes=true;window.discoveryPending=false;window.releaseSpaces();});
    await page.waitForFunction(()=>window.typeRequests.length>0);
    assert.equal(await types.isEnabled(),true);assert.equal(await spaces.isEnabled(),true);
    assert.match(await page.locator('#object-mentions-status').innerText(),/Loading object types/);
    await page.evaluate(()=>{window.holdTypes=false;window.releaseTypes();});
    await types.click();await typeMenu.getByRole('option',{name:'Task · Work',exact:true}).click();
    assert.equal(await typeMenu.getByRole('option',{name:'All',exact:true}).getAttribute('aria-selected'),'false');
    await page.keyboard.press('Escape');assert.equal(await types.innerText(),'1 selected');
    await spaces.click();await spaceMenu.getByRole('option',{name:'Personal',exact:true}).click();await page.keyboard.press('Escape');
    await types.click();const inactive=typeMenu.getByRole('option',{name:'Task · Work',exact:true});
    assert.match(await inactive.innerText(),/Inactive outside filtered spaces/);assert.equal(await inactive.getAttribute('aria-selected'),'true');
    assert.equal(await typeMenu.getByRole('option',{name:'Task · Personal',exact:true}).count(),1);
    await typeMenu.getByRole('option',{name:'Task · Personal',exact:true}).click();await page.keyboard.press('Escape');
    assert.equal(await page.evaluate(()=>window.saved),null);
    await incomplete.check();
    await page.locator('#save-button').click();await page.waitForFunction(()=>window.saved?.codexIntegration?.objectMentions.types.items.length===2);
    assert.equal(await page.evaluate(()=>window.saved.codexIntegration.objectMentions.filterIncompleteTasks),true);
    await incomplete.uncheck();await page.evaluate(()=>window.BlobUI.discardChanges());assert.equal(await incomplete.isChecked(),true);
    assert.deepEqual(await page.evaluate(()=>window.saved.codexIntegration.objectMentions.spaces),{all:false,items:[{id:'b',name:'Personal'}]});
    await types.click();await typeMenu.getByRole('option',{name:'All',exact:true}).click();await page.keyboard.press('Escape');
    assert.equal(await types.innerText(),'All');
    await page.evaluate(()=>window.BlobUI.discardChanges());assert.equal(await types.innerText(),'2 selected');
    await page.evaluate(()=>{window.spaceError=true;window.typeError=true;});await page.getByRole('button',{name:'Reload options',exact:true}).click();
    await page.waitForFunction(()=>document.getElementById('object-mentions-status').textContent.includes('space discovery failed'));
    assert.equal(await spaces.isEnabled(),true);assert.equal(await types.isEnabled(),true);
    await types.click();assert.equal(await inactive.getAttribute('aria-selected'),'true');await page.keyboard.press('Escape');
    await page.evaluate(()=>{window.spaceError=false;window.fixtureSpaces=[{id:'b',name:'Personal'}];});await page.getByRole('button',{name:'Reload options',exact:true}).click();
    await page.waitForFunction(()=>document.getElementById('object-mentions-status').textContent.includes('type discovery failed'));
    assert.equal(await types.isEnabled(),true);assert.equal(await spaces.isEnabled(),true);
    await page.evaluate(()=>{window.typeError=false;window.omitTypes=true;});await page.getByRole('button',{name:'Reload options',exact:true}).click();
    await types.click();assert.match(await typeMenu.getByRole('option',{name:'Task · Personal',exact:true}).innerText(),/Unavailable saved selection/);await page.keyboard.press('Escape');
    await spaces.click();await spaceMenu.getByRole('option',{name:'All',exact:true}).click();await page.keyboard.press('Escape');
    await page.evaluate(()=>{window.omitTypes=false;window.fixtureSpaces.push({id:'c',name:'New space'});});await page.getByRole('button',{name:'Reload options',exact:true}).click();
    await page.waitForFunction(()=>window.typeRequests.at(-1).includes('c'));
    assert.equal(await spaces.innerText(),'All');
    assert.equal(await page.evaluate(()=>window.saved.codexIntegration.objectMentions.spaces.all),false);
    await page.evaluate(()=>window.BlobUI.discardChanges());
    await page.screenshot({path:path.join(fixture,'settings-dark.png'),fullPage:true});
    await page.locator('#advanced-nav').click();await page.locator('#advanced-experiments-tab').click();await page.locator('#settings-v21-enabled').check();
    await page.locator('#codex-nav').click();await page.locator('#codex-section-links .v21-section-link').first().click();
    assert.equal(await spaces.isVisible(),true);await page.setViewportSize({width:620,height:950});
    await page.screenshot({path:path.join(fixture,'settings-refreshed-dark.png'),fullPage:true});
    assert.equal(await page.evaluate(()=>document.querySelector('.page-content').scrollWidth<=document.querySelector('.page-content').clientWidth),true);
    await page.evaluate(()=>{
      window.fixtureSpaces.push(...Array.from({length:40},(_,i)=>({id:'private-id-'+i,name:'Space '+String(i).padStart(2,'0')})));
      window.fixtureSpaces.push({id:'long',name:'School Notebook Entry · Private '.repeat(15)});
    });
    await page.getByRole('button',{name:'Reload options',exact:true}).click();await spaces.click();
    await spaceMenu.getByRole('option',{name:'Space 00',exact:true}).waitFor();
    await page.waitForFunction(()=>document.querySelector('#object-mentions-group [role=listbox]:not([hidden])')?.dataset.state==='open');
    await spaceMenu.evaluate(menu=>Promise.all(menu.getAnimations().map(animation=>animation.finished)));
    assert.doesNotMatch(await spaceMenu.innerText(),/private-id-/);
    assert.equal(await spaceMenu.locator('.kanban-select-option-description').count(),0);
    const longLabel=spaceMenu.getByRole('option',{name:/School Notebook Entry/}).locator('.kanban-option-label');
    assert.deepEqual(await longLabel.evaluate(label=>({nowrap:getComputedStyle(label).whiteSpace,overflow:getComputedStyle(label).textOverflow,clipped:label.scrollWidth>label.clientWidth})),{nowrap:'nowrap',overflow:'ellipsis',clipped:true});
    await spaceMenu.evaluate(menu=>{menu.scrollTop=120;menu.dispatchEvent(new Event('scroll'));});
    assert.equal(await spaceMenu.evaluate(menu=>menu.classList.contains('has-overflow')&&menu.classList.contains('is-scrolling')),true);
    const scrollbar=await spaceMenu.evaluate(menu=>({
      width:getComputedStyle(menu,'::-webkit-scrollbar').width,
      radius:getComputedStyle(menu,'::-webkit-scrollbar-thumb').borderRadius,
      buttons:getComputedStyle(menu,'::-webkit-scrollbar-button').display,
      color:getComputedStyle(menu).getPropertyValue('--page-scrollbar-color').trim()
    }));
    assert.equal(scrollbar.width,'12px');assert.equal(scrollbar.radius,'6px');assert.equal(scrollbar.buttons,'none');
    await page.screenshot({path:path.join(fixture,'picker-dark.png')});
    await page.waitForFunction(()=>!document.querySelector('#object-mentions-group [role=listbox]:not([hidden])').classList.contains('is-scrolling'));
    await page.keyboard.press('Escape');
    assert.deepEqual(errors,[]);
  }finally{await browser.close();}
});
