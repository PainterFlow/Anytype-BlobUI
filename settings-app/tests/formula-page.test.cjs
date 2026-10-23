const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const workspace = path.resolve(__dirname, '../..');
const root = path.join(workspace, '.test-run/kanban-live');
const executablePath = process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';

test('real-view calculator workflow, keyboard, drafts, saved refresh, and failure recovery', async () => {
  fs.mkdirSync(path.join(root, 'temp'), { recursive: true });
  process.env.TEMP = process.env.TMP = path.join(root, 'temp');
  const browser = await chromium.launch({ executablePath, headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  const page = await browser.newPage({ viewport: { width: 1050, height: 1000 } });
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(() => {
    const configuration = () => JSON.parse(sessionStorage.getItem('kanban-config') || '{"version":2,"display":"balanced","definitions":[]}');
    const target = viewId => ({ spaceId: 'space', objectId: 'object-a', blockId: 'board', viewId });
    window.__calls = []; window.__saves = [];
    window.__TAURI__ = { core: { invoke: async (command, args) => {
      window.__calls.push(command);
      if (command === 'get_app_state') return { ready: true, theme: document.documentElement.dataset.theme || 'dark', onboardingComplete: true, settings: {}, hasSavedApiKey: true };
      if (command === 'load_kanban_formulas') {
        if (sessionStorage.getItem('malformed')) throw Error('Malformed file');
        return { configuration: configuration(), calculatedAt: sessionStorage.getItem('calculated-at') };
      }
      if (command === 'discover_kanban_views') {
        if (window.__missingKey) throw Error('Add an Anytype API key in Advanced Settings to use Kanban formulas.');
        return { views: ['view-a', 'view-b'].filter(id => !window.__hideView || id !== 'view-a').map(viewId => ({ target: target(viewId), name: 'Board', space: 'Work', source: viewId === 'view-b' ? 'Tasks' : 'Launch plan', kind: viewId === 'view-b' ? 'Query' : 'Collection', properties: window.__hideProperty ? [] : [{ key: 'estimate', name: 'Estimate', format: 'number' }], unsupported: null })), warning: null };
      }
      if (command === 'evaluate_kanban_formula') {
        window.__evaluated = args;
        if (window.__defer) return new Promise(resolve => window.__resolve = resolve);
        return { categories: 4 };
      }
      if (command === 'save_kanban_formulas') {
        if (window.__failSave) throw Error('The board is unavailable.');
        window.__saves.push(args.configuration); sessionStorage.setItem('kanban-config', JSON.stringify(args.configuration)); sessionStorage.removeItem('malformed');
        sessionStorage.setItem('calculated-at', '2026-10-23T12:00:00Z'); return { calculatedAt: '2026-10-23T12:00:00Z', categories: 4 };
      }
      if (command === 'refresh_kanban_results') {
        if (window.__failRefresh) throw Error('Anytype is unavailable.');
        sessionStorage.setItem('calculated-at', '2026-10-23T13:00:00Z'); return { calculatedAt: '2026-10-23T13:00:00Z', categories: 4 };
      }
      if (command === 'get_calendar_api_availability') return 'available';
      if (command === 'save_settings') { if (window.__failGeneral) throw Error('General save failed'); return { calendarStatus: 'disabled' }; }
      return null;
    } } };
  });
  const panel = page.locator('.formula-view').first();
  const display = panel.getByRole('textbox');
  const open = async () => { await page.locator('#formulas-nav').click(); await page.locator('.formula-view-toggle').first().click(); };
  try {
    await page.goto(pathToFileURL(path.join(workspace, 'settings-app/ui/index.html')).href);
    await page.waitForFunction(() => window.appReady && !document.getElementById('formula-fields').disabled);
    await open();
    assert.equal(await page.locator('.formula-view').count(), 2);
    assert.equal(await panel.locator('.formula-key').count(), 24);
    assert.equal(await page.locator('[id*=formula-template], .formula-board, .formula-card-editor').count(), 0);
    await page.locator('#formula-search').fill('query'); assert.equal(await page.locator('.formula-view:visible').count(), 1);
    await page.locator('#formula-search').fill('nothing'); assert.match(await page.locator('#formula-no-matches').innerText(), /No matching/);
    await page.locator('#formula-search').fill('');
    await panel.getByRole('button', { name: 'Insert property', exact: true }).click();
    assert.equal(await panel.getByLabel('Aggregation', { exact: true }).isVisible(), false);
    await panel.getByLabel('Property', { exact: true }).selectOption('estimate');
    await panel.getByLabel('Aggregation', { exact: true }).selectOption('average');
    await panel.getByRole('button', { name: 'Insert', exact: true }).click();
    assert.match(await display.innerText(), /Average Estimate/);
    await page.keyboard.press('/'); await page.keyboard.press('2'); await page.keyboard.press('Enter');
    await panel.locator('.formula-calculation-status').filter({ hasText: /valid for 4 categories/ }).waitFor();
    assert.equal(await page.evaluate(() => window.__evaluated.tokens[0].property), 'estimate');
    assert.equal(await page.evaluate(() => window.__evaluated.tokens[0].operation), 'average');
    await page.locator('#preferences-nav').click(); await page.locator('#formulas-nav').click();
    assert.match(await display.innerText(), /Average Estimate/);
    await page.locator('#save-button').click(); await page.waitForFunction(() => window.__saves.length === 1);
    assert.equal(await page.locator('#formula-refresh-results').isEnabled(), true);
    await page.evaluate(() => window.__hideProperty = true); await page.locator('#formula-reload').click();
    await panel.locator('.formula-calculation-status').filter({ hasText: /saved property is unavailable/ }).waitFor();
    assert.equal(await page.evaluate(() => window.BlobUI.FormulaPage.read().definitions[0].tokens[0].property), 'estimate');
    await page.evaluate(() => window.__hideView = true); await page.locator('#formula-reload').click();
    await page.getByRole('button', { name: /Unavailable view/ }).waitFor();
    assert.equal(await page.evaluate(() => window.BlobUI.FormulaPage.read().definitions.length), 1);
    await page.evaluate(() => { window.__hideView = false; window.__hideProperty = false; }); await page.locator('#formula-reload').click();
    await panel.locator('.formula-calculation').filter({ hasText: /Average Estimate/ }).waitFor();
    await panel.getByRole('button', { name: 'Power', exact: true }).click(); await page.keyboard.press('3');
    assert.equal(await page.locator('#formula-refresh-results').isEnabled(), false);
    await page.evaluate(() => window.BlobUI.discardChanges());
    assert.doesNotMatch(await display.innerText(), /\^/);
    await panel.getByRole('button', { name: 'Clear calculation' }).click();
    await panel.getByRole('button', { name: 'Square root' }).click(); await page.keyboard.press('9'); await page.keyboard.press(')');
    await panel.getByRole('button', { name: 'Add', exact: true }).click();
    await panel.getByRole('button', { name: 'Nth root', exact: true }).click(); await page.keyboard.press('8'); await page.keyboard.press(','); await page.keyboard.press('3'); await page.keyboard.press(')');
    await panel.getByRole('button', { name: 'Backspace', exact: true }).click(); await page.keyboard.press(')');
    await page.evaluate(() => window.__failSave = true);
    await page.locator('#save-button').click(); await page.locator('#formula-storage-message').filter({ hasText: /pending edits/ }).waitFor();
    assert.equal(await page.locator('#save-button').isVisible(), true);
    await page.evaluate(() => window.__failSave = false); await page.locator('#save-button').click(); await page.waitForFunction(() => window.__saves.length === 2);
    await page.evaluate(() => { window.__failRefresh = true; });
    const before = await page.locator('#formula-calculated-at').innerText();
    await page.locator('#formula-refresh-results').click(); await page.locator('#formula-storage-message').filter({ hasText: /Previous published results are unchanged/ }).waitFor();
    assert.equal(await page.locator('#formula-calculated-at').innerText(), before);
    await page.evaluate(() => { window.__failRefresh = false; }); await page.locator('#formula-refresh-results').click();
    await page.locator('#formula-storage-message').filter({ hasText: /Results refreshed/ }).waitFor();
    await display.focus(); await page.keyboard.press('+'); await page.keyboard.press('2');
    await page.locator('#preferences-nav').click(); await page.locator('#collapsed-categories').check();
    await page.evaluate(() => window.__failGeneral = true); await page.locator('#save-button').click();
    await page.waitForFunction(() => window.__saves.length === 3 && !document.getElementById('save-button').disabled);
    await page.evaluate(() => { window.BlobUI.discardChanges(); window.__failGeneral = false; });
    assert.equal(await page.locator('#collapsed-categories').isChecked(), false);
    assert.equal(await page.evaluate(() => window.BlobUI.FormulaPage.read().definitions[0].tokens.at(-1).value), '2');
    await page.locator('#formulas-nav').click();
    await page.evaluate(() => window.__defer = true); await panel.getByRole('button', { name: 'Calculate categories' }).click();
    await display.focus(); await page.keyboard.press('+'); await page.keyboard.press('1');
    await page.evaluate(() => { window.__resolve({ categories: 99 }); window.__defer = false; });
    assert.doesNotMatch(await panel.locator('.formula-calculation-status').innerText(), /99/);
    await page.evaluate(() => window.BlobUI.discardChanges());
    await page.reload(); await page.waitForFunction(() => window.appReady && !document.getElementById('formula-fields').disabled); await open();
    assert.match(await display.innerText(), /√/);
    await page.locator('#advanced-nav').click(); await page.getByRole('tab', { name: 'Experiments', exact: true }).click(); await page.locator('#settings-v21-enabled').check();
    assert.equal(await page.locator('#formulas-nav').isVisible(), false);
    await page.locator('#preferences-nav').click(); await page.getByRole('tab', { name: 'Kanban', exact: true }).click(); await page.locator('#open-formulas').click();
    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.screenshot({ path: path.join(root, 'calculator-dark.png') });
    await page.evaluate(() => document.documentElement.dataset.theme = 'light'); await page.setViewportSize({ width: 800, height: 740 });
    await page.screenshot({ path: path.join(root, 'calculator-light-narrow.png') });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.evaluate(() => window.__missingKey = true); await page.locator('#formula-reload').click();
    await page.locator('#formula-storage-message').filter({ hasText: /API key/ }).waitFor();
    await page.evaluate(() => { sessionStorage.setItem('malformed', 'true'); }); await page.reload();
    await page.waitForFunction(() => window.appReady && !document.getElementById('formula-fields').disabled);
    await open(); assert.match(await page.locator('#formula-storage-message').innerText(), /original file is unchanged/);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
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
