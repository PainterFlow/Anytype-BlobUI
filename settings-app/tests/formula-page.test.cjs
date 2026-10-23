const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const workspace = path.resolve(__dirname, '../..');
const root = path.join(workspace, '.test-run/kanban-live');
const executablePath = process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';

test('calculator modal, fraction editing, view filters, persistence, and recovery', async () => {
  fs.mkdirSync(path.join(root, 'temp'), { recursive: true });
  process.env.TEMP = process.env.TMP = path.join(root, 'temp');
  const browser = await chromium.launch({ executablePath, headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  const page = await browser.newPage({ viewport: { width: 1050, height: 1000 } });
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    const configuration = () => JSON.parse(sessionStorage.getItem('kanban-config') || '{"version":2,"display":"balanced","definitions":[]}');
    const target = viewId => ({ spaceId: viewId === 'view-b' ? 'spc002' : 'spc001', objectId: 'object-a', blockId: 'board', viewId });
    window.__calls = []; window.__saves = [];
    window.__TAURI__ = { core: { invoke: async (command, args) => {
      window.__calls.push(command);
      if (command === 'get_app_state') return { ready: true, theme: document.documentElement.dataset.theme || 'dark', onboardingComplete: true, settings: {}, hasSavedApiKey: true };
      if (command === 'load_kanban_formulas') return { configuration: configuration(), calculatedAt: sessionStorage.getItem('calculated-at') };
      if (command === 'discover_kanban_views') {
        return { views: ['view-a', 'view-b'].filter(id => !window.__hideView || id !== 'view-a').map(viewId => ({
          target: target(viewId), name: 'Board', space: 'Work', source: viewId === 'view-b' ? 'Tasks' : 'Launch plan',
          sourceSignature: viewId === 'view-b' ? 'source-signature-b' : 'source-signature-a',
          kind: viewId === 'view-b' ? 'Query' : 'Collection',
          properties: window.__hideProperty ? [] : window.__propertyList && viewId === 'view-a' ? window.__propertyList : viewId === 'view-b'
            ? [{ key: 'notes', name: 'Notes', format: 'text' }, { key: 'due-date', name: 'Due date', format: 'date' }]
            : [
              { key: 'estimate', name: 'Estimate', format: 'number' },
              { key: 'notes', name: 'Notes', format: 'text' },
              { key: 'due-date', name: 'Due date', format: 'date' },
            ], unsupported: null,
        })).flatMap(view=>view.target.viewId === "view-a" ? [view,{...view,target:{...view.target,blockId:"inline-copy"},kind:"Inline / type view"}] : [view]), warning: null };
      }
      if (command === 'evaluate_kanban_formula') throw Error('Calculator edits must not evaluate on every key.');
      if (command === 'save_kanban_formulas') {
        if (window.__failSave) throw Error('The board is unavailable.');
        window.__saves.push(args.configuration); sessionStorage.setItem('kanban-config', JSON.stringify(args.configuration));
        sessionStorage.setItem('calculated-at', '2026-10-23T12:00:00Z');
        return { calculatedAt: '2026-10-23T12:00:00Z', categories: 4 };
      }
      if (command === 'get_calendar_api_availability') return 'available';
      if (command === 'save_settings') return { calendarStatus: 'disabled' };
      return null;
    } } };
  });
  const modal=page.locator('#formula-calculator-dialog');
  const key=name=>modal.getByRole('button',{name,exact:true});
  const open=async()=>{await page.locator('.formula-view-row').filter({hasText:'Launch plan'}).click();await modal.waitFor({state:'visible'});};
  const close=async()=>{await key('Save calculation').click();await modal.waitFor({state:'hidden'});};
  const read=()=>page.evaluate(()=>window.BlobUI.FormulaPage.read());
  try {
    await page.goto(pathToFileURL(path.join(workspace,'settings-app/ui/index.html')).href);
    await page.waitForFunction(()=>window.appReady);
    await page.waitForFunction(()=>document.querySelector('#status-message').textContent.includes('Settings loaded from this device.'));
    await page.waitForFunction(()=>window.__calls.includes('discover_kanban_views'));
    await page.locator('.formula-view-row').first().waitFor({state:'attached'});
    assert.equal(await page.locator('#formulas-page').isVisible(),false,'Startup discovery runs before opening the formula page.');
    const initialDiscoveries=await page.evaluate(()=>window.__calls.filter(c=>c==='discover_kanban_views').length);
    await page.evaluate(()=>{
      window.BlobUI.FormulaPage.setLocked(true,false);
      window.BlobUI.FormulaPage.reload();
      queueMicrotask(()=>window.BlobUI.FormulaPage.setLocked(false,false));
    });
    await page.waitForFunction(n=>window.__calls.filter(c=>c==='discover_kanban_views').length>n,initialDiscoveries);
    await page.waitForFunction(()=>!document.getElementById('formula-reload').disabled);
    assert.equal(await page.evaluate(()=>window.__calls.filter(c=>c==='discover_kanban_views').length),initialDiscoveries+1,'A reload queued while locked flushes once when an immediate unlock happens.');
    assert.equal(await page.locator('#formulas-page').isVisible(),false,'Unlock retries queued discovery while the formula page is hidden.');
    await page.locator('#formulas-nav').click();await page.locator('.formula-view-row').first().waitFor();
    assert.equal(await page.locator('.formula-intro,#formula-calculated-at').count(),0);
    assert.equal(await page.locator('.formula-view-row').count(),2);
    const rows=await page.locator('.formula-view-row').evaluateAll(nodes=>nodes.map(n=>({font:getComputedStyle(n.querySelector('.formula-view-name')).fontSize,radius:getComputedStyle(n).borderRadius})));
    assert.equal(rows[0].font,rows[1].font);assert.notEqual(rows[0].radius,'0px');
    assert.equal(await page.locator('.formula-view').first().evaluate(n=>getComputedStyle(n).borderTopWidth),'0px');
    await page.locator('#formula-filter').click();
    await page.locator('#formula-filter-options [data-value="spc001"]').click();
    assert.equal(await page.locator('#formula-filter').getAttribute('aria-expanded'),'true');
    await page.locator('#formula-filter-options [data-value="spc002"]').click();
    assert.equal(await page.locator('#formula-filter-options [data-value="spc001"]').getAttribute('aria-selected'),'false');
    assert.equal(await page.locator('.formula-view-row:visible').count(),1);
    await page.locator('#formula-filter-options [data-value=""]').click();
    await page.keyboard.press('Escape');
    await page.locator('#formula-search-toggle').click();await page.locator('#formula-search').fill('Launch');
    assert.equal(await page.locator('.formula-view-row:visible').count(),1);
    assert.equal(await page.locator('#formula-search').evaluate(n=>getComputedStyle(n).borderTopWidth),'0px');
    await page.locator('#formula-search').fill('');await page.locator('#page-title').click();
    await open();
    assert.equal(await modal.locator('select').count(),0);
    assert.deepEqual(await modal.locator('.formula-variable-actions').evaluate(row=>[...row.children].map(node=>node.classList.contains('formula-property-picker')?'property':node.classList.contains('formula-variable-button')?'count':node.classList.contains('formula-render-toggle')?'latex':'remove')),['property','count','remove','latex']);
    assert.equal(await key('Remove calculation').isVisible(),true,'The trash control stays in the action row when the formula is empty.');
    assert.equal(await key('Remove calculation').isDisabled(),true,'The empty formula cannot be removed.');
    assert.doesNotMatch(await modal.locator('.formula-display').innerText(),/^[xy]$/);
    assert.ok(await modal.locator('.formula-key[aria-label="Backspace"] svg').count());
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(()=>document.activeElement.classList.contains('formula-display')),true,'Tab stays on the calculation editor in rendered mode.');
    assert.equal(await page.evaluate(()=>document.activeElement.tagName), 'DIV');
    await key('1').click();await key('2').click();await key('3').click();await key('Backspace').click();await key('4').click();
    assert.equal((await read()).definitions[0].tokens[0].value,'124');
    await key('Clear calculation').click();
    assert.equal(await key('Remove calculation').isDisabled(),true);
    await key('Fraction').click();
    assert.equal(await key('Remove calculation').isDisabled(),true,'An empty fraction does not count as input.');
    await key('2').click();
    assert.equal(await key('Remove calculation').isDisabled(),false,'A partial local fraction counts as input.');
    await key('Remove calculation').click();
    assert.equal(await key('Remove calculation').isDisabled(),true,'Removing a partial fraction clears its draft.');
    await key('Insert property').click();
    assert.deepEqual(await modal.locator('.formula-property-picker [role="option"] .kanban-option-label').allTextContents(),['Estimate'],'Only numeric properties are offered.');
    assert.equal(await modal.locator('.formula-property-picker [role="option"] .kanban-select-option-description').textContent(),'\\property{"Estimate"}');
    await key('Insert property').click();
    await key('1').click();await key('2').click();
    await key('Insert property').click();
    await modal.getByRole('option',{name:'Estimate',exact:true}).click();
    assert.deepEqual((await read()).definitions[0].tokens.map(item=>item.kind==='operator'?item.value:item.kind),['number','*','aggregate'],'Rendered insertion joins adjacent values with multiplication.');
    await key('Clear calculation').click();
    await key('Insert property').click();
    await modal.getByRole('option',{name:'Estimate',exact:true}).click();
    assert.equal((await read()).definitions[0].tokens[0].property,'estimate');
    assert.equal((await read()).definitions[0].tokens[0].operation,'total');
    assert.equal(await key('Remove calculation').isDisabled(),false);
    assert.equal(await page.evaluate(()=>document.activeElement.classList.contains('formula-display')),true,'Mouse property selection returns focus to the calculation editor.');
    await key('Fraction').click();
    await key('Insert property').click();
    await modal.getByRole('option',{name:'Estimate',exact:true}).click();
    assert.equal(await modal.locator('mfrac').count(),1);
    await key('Insert property').click();
    assert.equal(await modal.locator('.formula-property-picker [role="option"]').count(),1,'The property picker can reopen after an insertion inside a fraction.');
    await key('Insert property').click();
    assert.equal(await modal.locator('.formula-insert-property').getAttribute('aria-expanded'),'false','The property menu can be closed again after reopening.');
    assert.equal((await read()).definitions[0].tokens[5].property,'estimate');
    await modal.locator('.formula-display').focus();await page.keyboard.press('ArrowUp');
    assert.equal(await modal.locator('.formula-display').getAttribute('data-active'),'numerator');
    const renderedBox=await modal.locator('.formula-display-slot').boundingBox();
    await key('Show LaTeX').click();
    const source=modal.getByRole('textbox',{name:'LaTeX source'});
    assert.equal(await page.evaluate(()=>document.activeElement.getAttribute('aria-label')),'LaTeX source');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(()=>document.activeElement.getAttribute('aria-label')),'LaTeX source','Tab does not leave the native source editor for calculator buttons.');
    await page.keyboard.press('Tab');
    assert.equal(await page.evaluate(()=>document.activeElement.getAttribute('aria-label')),'LaTeX source','Tab cycles between the editor and source without landing on buttons.');
    const sourceBox=await modal.locator('.formula-display-slot').boundingBox();
    assert.ok(Math.abs(renderedBox.height-sourceBox.height)<0.5 && Math.abs(renderedBox.width-sourceBox.width)<0.5,'Rendered and source modes reserve the same display box.');
    assert.match(await source.inputValue(),/\\frac/);
    const valid=await read();await source.fill('\\frac{1}{');
    assert.match(await modal.locator('.formula-calculator-utility .formula-calculator-status').innerText(),/brace|calculation|number/i);
    assert.deepEqual(await read(),valid);
    await source.evaluate(node=>node.setSelectionRange(0,0));
    await key('Insert property').click();
    await modal.getByRole('option',{name:'Estimate',exact:true}).click();
    assert.equal(await source.inputValue(),'\\property{"Estimate"}\\frac{1}{','Source insertion preserves invalid text and replaces only the current selection.');
    assert.equal(await source.evaluate(node=>node.selectionStart),'\\property{"Estimate"}'.length,'Source insertion leaves the caret after the inserted reference.');
    assert.equal(await page.evaluate(()=>document.activeElement.getAttribute('aria-label')),'LaTeX source');
    assert.equal(await key('Remove calculation').isDisabled(),false,'Nonempty invalid source can still be cleared.');
    await key('Remove calculation').click();
    assert.equal(await modal.locator('.formula-latex-input').evaluate(node=>node.value),'','Removing invalid source clears the draft text.');
    assert.equal(await key('Remove calculation').isDisabled(),true,'Removing invalid source returns the trash action to its empty state.');
    await key('Show LaTeX').click();
    await source.fill('\\frac{1}{');
    await source.evaluate(node=>node.setSelectionRange(0,0));
    await key('Insert property').click();
    await modal.getByRole('option',{name:'Estimate',exact:true}).click();
    assert.equal(await source.inputValue(),'\\property{"Estimate"}\\frac{1}{');
    const nativeWrites=await page.evaluate(()=>({settings:window.__calls.filter(c=>c==='save_settings').length,formulas:window.__saves.length}));
    const sourceDraft=await source.inputValue(), stateDraft=await read();
    const beforeCtrlR=await page.evaluate(()=>window.__calls.filter(c=>c==='discover_kanban_views').length);
    await page.keyboard.press('Control+r');
    await page.waitForFunction(n=>window.__calls.filter(c=>c==='discover_kanban_views').length>n,beforeCtrlR);
    assert.equal(await modal.isVisible(),true,'Ctrl+R leaves the calculator open.');
    assert.equal(await source.inputValue(),sourceDraft,'Ctrl+R preserves unsaved LaTeX text.');
    assert.deepEqual(await read(),stateDraft,'Ctrl+R preserves the calculator draft.');
    assert.deepEqual(await page.evaluate(()=>({settings:window.__calls.filter(c=>c==='save_settings').length,formulas:window.__saves.length})),nativeWrites,'Editing and reload do not write native settings.');
    await close();assert.equal(await page.evaluate(()=>window.__saves.length),0);
    assert.equal(await page.evaluate(()=>window.__calls.includes('save_settings')),false);
    await open();assert.equal(await source.inputValue(),'\\property{"Estimate"}\\frac{1}{');
    await source.fill('1+2');
    await source.evaluate(node=>node.setSelectionRange(1,1));
    await key('Insert property').click();
    await modal.getByRole('option',{name:'Estimate',exact:true}).click();
    assert.equal(await source.inputValue(),'1*\\property{"Estimate"}+2','Insertion before an addition operator does not add an unnecessary multiplication.');
    await source.fill('12');
    await source.evaluate(node=>node.setSelectionRange(1,1));
    await key('Insert property').click();
    await modal.getByRole('option',{name:'Estimate',exact:true}).click();
    assert.equal(await source.inputValue(),'1*\\property{"Estimate"}*2','LaTeX insertion joins both sides of a mid-number caret.');
    assert.equal(await source.evaluate(node=>node.selectionStart),'1*\\property{"Estimate"}'.length);
    assert.equal(await page.evaluate(()=>document.activeElement.getAttribute('aria-label')),'LaTeX source');
    await source.fill('12');
    await source.evaluate(node=>node.setSelectionRange(1,1));
    await key('Card count').click();
    assert.equal(await source.inputValue(),'1*\\operatorname{count}\\left(\\text{cards}\\right)*2','Card count insertion preserves the source mode and uses the selected caret.');
    await source.fill('\\frac{8}{2}+{3}^{2}');await key('Show rendered').click();
    assert.ok(await modal.locator('mfrac').count()>0);assert.ok(await modal.locator('msup').count()>0);
    await page.screenshot({path:path.join(root,'calculator-modal-dark.png')});
    for(const size of [{width:800,height:740},{width:640,height:600}]){
      await page.setViewportSize(size);
      const bounds=await modal.boundingBox();assert.ok(bounds.y>=0&&bounds.y+bounds.height<=size.height);
      assert.equal(await modal.evaluate(n=>n.scrollHeight<=n.clientHeight),true);
      assert.ok(Math.abs(bounds.x+bounds.width/2-size.width/2)<2);
    }
    await page.evaluate(()=>document.documentElement.dataset.theme='light');await page.screenshot({path:path.join(root,'calculator-modal-light-narrow.png')});
    await page.setViewportSize({width:1050,height:1000});await page.evaluate(()=>document.documentElement.dataset.theme='dark');
    await close();assert.equal(await page.evaluate(()=>window.__saves.length),0);
    await page.evaluate(()=>window.BlobUI.discardChanges());assert.equal((await read()).definitions.length,0);
    await open();assert.equal(await modal.locator('msup').count(),0);await key('4').click();await close();
    await page.locator('#save-button').click();await page.waitForFunction(()=>window.__saves.length===1);
    await open();await key('Add').click();await key('2').click();await close();
    await page.evaluate(()=>window.__failSave=true);await page.locator('#save-button').click();
    await page.locator('#formula-storage-message').filter({hasText:/pending edits/}).waitFor();
    assert.equal(await page.evaluate(()=>window.__saves.length),1);
    await page.evaluate(()=>window.__failSave=false);await page.locator('#save-button').click();await page.waitForFunction(()=>window.__saves.length===2);
    assert.equal(await page.evaluate(()=>window.__calls.includes('evaluate_kanban_formula')),false);
    const before=await page.evaluate(()=>window.__calls.filter(c=>c==='discover_kanban_views').length);
    await page.locator('#formula-reload').click();await page.waitForFunction(n=>window.__calls.filter(c=>c==='discover_kanban_views').length>n,before);
    await page.locator('.formula-view-row').filter({hasText:'Tasks'}).click();await modal.waitFor({state:'visible'});
    await key('Insert property').click();
    assert.equal(await modal.locator('.formula-property-picker [role="option"]').count(),0,'An empty numeric property list adds no fake option.');
    assert.equal(await modal.locator('.formula-property-picker [role="status"]').innerText(),'No number or boolean properties in this view');
    await key('Save calculation').click();await modal.waitFor({state:'hidden'});
    await page.evaluate(()=>{window.__propertyList=[
      {key:'dup-a',name:'Estimate',format:'checkbox'},
      {key:'dup-z',name:'Estimate',format:'number'},
      {key:'done',name:'Done',format:'boolean'},
    ];});
    const beforeProperties=await page.evaluate(()=>window.__calls.filter(c=>c==='discover_kanban_views').length);
    await page.locator('#formula-reload').click();
    await page.waitForFunction(n=>window.__calls.filter(c=>c==='discover_kanban_views').length>n,beforeProperties);
    await open();
    await key('Insert property').click();
    assert.deepEqual(await modal.locator('.formula-property-picker [role="option"] .kanban-option-label').allTextContents(),['Estimate','Estimate','Done']);
    assert.deepEqual(await modal.locator('.formula-property-picker [role="option"] .kanban-select-option-description').allTextContents(),[
      '\\property{"Estimate"}#1','\\property{"Estimate"}#2','\\property{"Done"}',
    ]);
    await modal.locator('.formula-property-picker [role="option"]').nth(1).click();
    const duplicateToken=await page.evaluate(()=>window.BlobUI.FormulaPage.read().definitions[0].tokens.find(item=>item.kind==='aggregate'&&item.property==='dup-z'));
    assert.equal(duplicateToken.operation,'total','Choosing a duplicate property inserts its stable numbered reference.');
    await key('Show LaTeX').click();
    assert.equal(await modal.locator('.formula-render-toggle svg.visibility-icon').count(),1,'The source toggle reuses the settings eye icon.');
    assert.equal(await modal.getByRole('button',{name:'Show rendered',exact:true}).getAttribute('aria-pressed'),'true');
    await key('Show rendered').click();
    await key('Save calculation').click();await modal.waitFor({state:'hidden'});
    await page.screenshot({path:path.join(root,'calculator-toolbar-filter-search.png')});
    assert.deepEqual(errors,[]);
  } finally {await browser.close();}
});

test('native-generated CSS scopes exact view/category IDs and falls back on narrow headers', async () => {
  const browser = await chromium.launch({ executablePath, headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  const page = await browser.newPage();
  const board = object => `<div data-root-id="${object}"><div id="block-board"><div id="views" style="display:none"><div id="view-item-board-view-a" class="viewItem active"></div></div><div class="viewContent viewBoard"><div id="columns">${['option-a', 'empty', 'other'].map(id => `<div id="column-${id}" class="column"><div class="head"><div class="sides"><div class="side left"><span>Category</span></div><div class="side right"><button>More</button></div></div></div></div>`).join('')}</div></div></div></div>`;
  try {
    for (const mode of ['balanced', 'spacious', 'compact']) {
      await page.setContent(`<style>:root{--color-bg-primary:#202020;--color-text-primary:#eee;--color-shape-secondary:#555}body{font:14px sans-serif;color:#eee;background:#171717}.head{width:400px;margin:20px}.sides{display:flex;height:28px}.side.left{flex:1;overflow:hidden}.side.right{display:none}.column:hover .side.right{display:flex}</style>${board('object-a')}${board('object-b')}`);
      await page.addStyleTag({ content: fs.readFileSync(path.join(root, `${mode}.css`), 'utf8') });
      const correct = page.locator('[data-root-id="object-a"] #column-option-a');
      assert.equal(await correct.locator('.side.right').evaluate(el => getComputedStyle(el).display), 'flex');
      const pseudo = selector => page.locator(selector).evaluate(el => getComputedStyle(el, '::after').content);
      if (mode !== 'compact') assert.match(await correct.locator('.side.left').evaluate(el => getComputedStyle(el, '::after').content), /12/);
      assert.equal(await pseudo('[data-root-id="object-b"] #column-option-a .side.left'), 'none');
      assert.equal(await pseudo('[data-root-id="object-a"] #column-other .side.left'), 'none');
      assert.match(await pseudo('[data-root-id="object-a"] #column-empty .side.left'), /No value/);
      await correct.locator('.head').evaluate(el => el.style.width = '100px');
      await page.waitForFunction(() => getComputedStyle(document.querySelector('[data-root-id="object-a"] #column-option-a .side.left'), '::after').display === 'none');
      await correct.locator('.head').hover();
      assert.equal(await correct.locator('.side.right').evaluate(el => getComputedStyle(el, '::after').visibility), 'visible');
      await page.locator('[data-root-id="object-a"] .viewItem').evaluate(el => el.classList.remove('active'));
      assert.equal(await pseudo('[data-root-id="object-a"] #column-option-a .side.left'), 'none');
    }
  } finally { await browser.close(); }
});
