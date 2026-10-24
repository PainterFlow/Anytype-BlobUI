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
    assert.equal(await key('Remove calculation').count(),0);
    const source=modal.getByRole('textbox',{name:'LaTeX source'});
    const styles=()=>key('1').evaluate(n=>{const s=getComputedStyle(n);return [s.opacity,s.color,s.backgroundColor,s.cursor,s.borderRadius];});
    const renderedStyles=await styles();
    assert.equal(await modal.locator('.formula-display').getAttribute('aria-readonly'),'true');
    for (const keyName of ['ArrowDown','ArrowUp','Enter',' ']) {
      await key('Insert property').dispatchEvent('keydown',{key:keyName});
      assert.equal(await key('Insert property').getAttribute('aria-expanded'),'false','Rendered keyboard shortcuts keep the property menu closed.');
    }
    await key('1').click();await key('Card count').click();await key('Clear calculation').click();
    await modal.locator('.formula-display').focus();await page.keyboard.type('123+4');await page.keyboard.press('Backspace');
    assert.equal((await read()).definitions.length,0,'Rendered controls and keyboard never mutate the formula.');
    assert.equal(await modal.locator('.formula-display').evaluate(n=>getComputedStyle(n).cursor),'default');
    assert.equal(await modal.locator('.formula-display .formula-caret:visible').count(),0);
    const renderedBox=await modal.locator('.formula-display-slot').boundingBox();
    await key('Show LaTeX').click();
    assert.deepEqual(await styles(),renderedStyles,'Keypad appearance stays identical in either mode.');
    const sourceBox=await modal.locator('.formula-display-slot').boundingBox();
    assert.ok(Math.abs(sourceBox.height-renderedBox.height)<0.5);
    await source.fill('12');await source.evaluate(n=>n.setSelectionRange(1,2));await key('3').click();
    assert.equal(await source.inputValue(),'13');
    await source.fill('\\frac{1}{'.replaceAll('\\\\','\\'));
    await source.evaluate(n=>n.setSelectionRange(0,0));await key('2').click();
    assert.equal(await source.inputValue(),'2\\frac{1}{'.replaceAll('\\\\','\\'),'Keypad preserves incomplete source.');
    await key('Clear calculation').click();assert.equal(await source.inputValue(),'');assert.equal((await read()).definitions.length,0);
    await key('Insert property').click();
    assert.deepEqual(await modal.locator('.formula-property-picker [role="option"] .kanban-option-label').allTextContents(),['Estimate']);
    await modal.getByRole('option',{name:'Estimate',exact:true}).click();
    assert.equal(await source.inputValue(),'','A property requires an explicit aggregation choice.');
    await modal.getByRole('option',{name:'Average',exact:true}).click();
    assert.equal(await source.inputValue(),'\\property{"Estimate"}{"average"}'.replaceAll('\\\\','\\'));
    assert.equal((await read()).definitions[0].tokens[0].operation,'average');
    await source.fill('12');await source.evaluate(n=>n.setSelectionRange(1,1));await key('Card count').click();
    assert.equal(await source.inputValue(),'1*\\count*2'.replaceAll('\\\\','\\'));
    await source.fill('\\frac{8}{2}+{3}^{2}'.replaceAll('\\\\','\\'));
    const valid=await read();await key('Show rendered').click();
    assert.ok(await modal.locator('mfrac').count()>0);assert.ok(await modal.locator('msup').count()>0);
    for (const name of ['1','Backspace','Clear calculation','Card count','Square root','Nth root','Fraction','Change sign']) await key(name).click();
    await key('Insert property').click();assert.equal(await key('Insert property').getAttribute('aria-expanded'),'false');
    await modal.locator('.formula-display').focus();await page.keyboard.press('ArrowDown');await page.keyboard.press('Delete');
    await modal.locator('.formula-display').dispatchEvent('paste');
    assert.deepEqual(await read(),valid,'Every rendered mutation path leaves the draft unchanged.');
    await key('Show LaTeX').click();assert.equal(await source.inputValue(),'\\frac{8}{2}+{3}^{2}'.replaceAll('\\\\','\\'));
    await source.fill('\\frac{1}{'.replaceAll('\\\\','\\'));
    await key('Show rendered').click();await key('Clear calculation').click();await key('Show LaTeX').click();
    assert.equal(await source.inputValue(),'\\frac{1}{'.replaceAll('\\\\','\\'),'Rendered C preserves invalid source.');
    await key('Clear calculation').click();assert.equal(await source.inputValue(),'');
    await source.fill('4');await close();assert.equal(await page.evaluate(()=>window.__saves.length),0);
    await page.locator('#save-button').click();await page.waitForFunction(()=>window.__saves.length===1);
    await open();await source.fill('4+2');await close();
    await page.evaluate(()=>window.__failSave=true);await page.locator('#save-button').click();
    await page.locator('#formula-storage-message').filter({hasText:/pending edits/}).waitFor();
    assert.equal(await page.evaluate(()=>window.__saves.length),1);
    await page.evaluate(()=>window.__failSave=false);await page.locator('#save-button').click();await page.waitForFunction(()=>window.__saves.length===2);
    await open();await source.fill('99');await close();await page.evaluate(()=>window.BlobUI.discardChanges());
    assert.equal((await read()).definitions[0].tokens[0].value,'4');
    assert.equal(await page.evaluate(()=>window.__calls.includes('evaluate_kanban_formula')),false);
    await open();await key('Show LaTeX').click();await source.fill('\\sqrt[3]{8}'.replaceAll('\\\\','\\'));await key('Show rendered').click();
    await page.screenshot({path:path.join(root,'calculator-modal-dark.png')});
    for(const size of [{width:800,height:740},{width:640,height:600}]){
      await page.setViewportSize(size);const bounds=await modal.boundingBox();assert.ok(bounds.y>=0&&bounds.y+bounds.height<=size.height);
      assert.equal(await modal.evaluate(n=>n.scrollHeight<=n.clientHeight),true);
    }
    await page.evaluate(()=>document.documentElement.dataset.theme='light');await page.screenshot({path:path.join(root,'calculator-modal-light-narrow.png')});
    await close();
    await page.setViewportSize({width:1050,height:1000});
    await page.locator('.formula-view-row').filter({hasText:'Tasks'}).click();await modal.waitFor({state:'visible'});
    await key('Show LaTeX').click();await key('Insert property').click();
    assert.equal(await modal.locator('.formula-property-picker [role="option"]').count(),0);
    assert.equal(await modal.locator('.formula-property-picker [role="status"]').innerText(),'No number or boolean properties in this view');
    await close();
    await page.evaluate(()=>{window.__propertyList=[{key:'dup-a',name:'Estimate',format:'checkbox'},{key:'dup-z',name:'Estimate',format:'number'},{key:'done',name:'Done',format:'boolean'}];});
    const beforeProperties=await page.evaluate(()=>window.__calls.filter(c=>c==='discover_kanban_views').length);
    await page.locator('#formula-reload').click();await page.waitForFunction(n=>window.__calls.filter(c=>c==='discover_kanban_views').length>n,beforeProperties);
    await open();await key('Show LaTeX').click();await key('Clear calculation').click();await key('Insert property').click();
    assert.deepEqual(await modal.locator('.formula-property-picker [role="option"] .kanban-option-label').allTextContents(),['Estimate','Estimate','Done']);
    assert.deepEqual(await modal.locator('.formula-property-picker [role="option"] .kanban-select-option-description').allTextContents(),['\\property{"Estimate"}#1','\\property{"Estimate"}#2','\\property{"Done"}']);
    await modal.locator('.formula-property-picker [role="option"]').nth(1).click();await modal.getByRole('option',{name:'Median',exact:true}).click();
    const duplicateToken=(await read()).definitions[0].tokens[0];assert.equal(duplicateToken.property,'dup-z');assert.equal(duplicateToken.operation,'median');
    const perCardPrefix='\\property{"Estimate"}#2{"sum"}{';
    await source.fill(perCardPrefix+'}');
    await source.evaluate((node,position)=>node.setSelectionRange(position,position),perCardPrefix.length);
    await key('Insert property').click();await modal.getByRole('option',{name:'Done',exact:true}).click();
    assert.equal(await source.inputValue(),perCardPrefix+'\\property{"Done"}}','An empty per-card expression inserts a bare property without aggregation.');
    assert.equal(await key('Insert property').getAttribute('aria-expanded'),'false');
    assert.deepEqual((await read()).definitions[0].tokens[0].calculation,[{kind:'property',property:'done'}]);
    const nested=perCardPrefix+'\\property{"Estimate"}#2 * \\property{"Done"} + \\property{"Estimate"}#1{"maximum"}}';
    await source.fill(nested);
    const nestedState=await read();
    await key('Show rendered').click();await key('1').click();await key('Clear calculation').click();await key('Card count').click();
    assert.deepEqual(await read(),nestedState,'A rendered per-card formula remains read only.');
    assert.ok(await modal.locator('math').count()>0);
    await key('Show LaTeX').click();assert.equal(await source.inputValue(),nested);
    await close();await open();assert.equal(await source.inputValue(),nested,'Closing and reopening retains the custom source.');
    await close();
    await page.locator('#save-button').click();await page.waitForFunction(()=>window.__saves.length===3);
    assert.deepEqual(await page.evaluate(()=>window.__saves.at(-1).definitions[0].tokens),nestedState.definitions[0].tokens);
    await open();
    await source.fill(perCardPrefix+'1+');
    await key('Insert property').click();await modal.getByRole('option',{name:'Done',exact:true}).click();
    assert.equal(await source.inputValue(),perCardPrefix+'1+\\property{"Done"}','Incomplete per-card source retains its text and scope.');
    await source.fill(nested);
    const replacementStart=nested.indexOf('\\property{"Done"}');
    await source.evaluate((node,range)=>node.setSelectionRange(...range),[replacementStart,replacementStart+'\\property{"Done"}'.length]);
    await key('Insert property').click();await modal.locator('.formula-property-picker [role="option"]').nth(0).click();
    assert.equal(await source.inputValue(),nested.replace('\\property{"Done"}','\\property{"Estimate"}#1'),'A selected nested property is replaced without changing the surrounding calculation.');
    await close();
    await page.evaluate(()=>{window.__propertyList=[{key:'price',name:'Price {net}',format:'number'},{key:'quantity',name:'Quantity',format:'number'}];});
    await page.locator('#formula-reload').click();
    await page.waitForFunction(()=>document.querySelector('#formula-reload').disabled===false);
    await open();
    assert.match(await modal.locator('#formula-calculator-status').innerText(),/unavailable/);
    const quotedPrefix='\\property{"Price {net}"}{"sum"}{\\property{"Price {net}"}';
    await source.fill(quotedPrefix+'}');
    await source.evaluate((node,position)=>node.setSelectionRange(position,position),quotedPrefix.length);
    await key('Insert property').click();await modal.getByRole('option',{name:'Quantity',exact:true}).click();
    assert.equal(await source.inputValue(),quotedPrefix+'*\\property{"Quantity"}}','Quoted braces do not hide a nested property boundary or its per-card scope.');
    await close();
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
