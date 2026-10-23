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
          kind: viewId === 'view-b' ? 'Query' : 'Collection',
          properties: window.__hideProperty ? [] : [{ key: 'estimate', name: 'Estimate', format: 'number' }], unsupported: null,
        })), warning: null };
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
  const openFormulaPage = async () => {
    await page.locator('#formulas-nav').click();
    await page.locator('.formula-view-row').first().waitFor();
  };
  const waitForToolbarMotion = async () => page.locator('.formula-toolbar').evaluate(async toolbar => {
    await Promise.all(toolbar.getAnimations({ subtree: true }).map(animation => animation.finished.catch(() => {})));
  });
  const openFilter = async () => {
    await page.locator('#formula-filter').click();
    await page.waitForFunction(() => document.getElementById('formula-filter-options').dataset.state === 'open');
    await waitForToolbarMotion();
    const menuBox = await page.locator('#formula-filter-options').boundingBox();
    const contentBox = await page.locator('#formulas-page').boundingBox();
    assert.ok(menuBox.x >= contentBox.x, 'The space menu stays inside the content left edge.');
    assert.ok(menuBox.x + menuBox.width <= page.viewportSize().width, 'The space menu stays inside the viewport.');
  };
  try {
    await page.goto(pathToFileURL(path.join(workspace, 'settings-app/ui/index.html')).href);
    await page.waitForFunction(() => window.appReady && !document.getElementById('formula-fields').disabled);
    await openFormulaPage();
    assert.equal(await page.locator('.formula-view-row').count(), 2);
    assert.equal(await page.locator('#formula-refresh-results, .formula-view-toggle').count(), 0);

    await openFilter();
    const options = page.locator('#formula-filter-options [role="option"]');
    await page.screenshot({ path: path.join(root, 'calculator-toolbar-filter.png') });
    assert.equal(await options.count(), 3);
    assert.deepEqual(await options.allInnerTexts(), ['All spaces', 'Work · spc001', 'Work · spc002']);
    await options.filter({ hasText: 'spc002' }).click();
    assert.equal(await page.locator('.formula-view-row:visible').count(), 1);
    assert.match(await page.locator('.formula-view-row:visible').innerText(), /Tasks/);
    await page.waitForFunction(() => document.getElementById('formula-filter-options').hidden);
    await page.locator('#formula-search-toggle').click();
    await page.waitForFunction(() => document.getElementById('formula-search-control').dataset.expanded === 'true');
    await waitForToolbarMotion();
    assert.equal(await page.locator('#formula-search-control').getAttribute('data-expanded'), 'true');
    assert.ok((await page.locator('#formula-search').boundingBox()).width > 200, 'The expanded search field finishes opening before capture.');
    await page.screenshot({ path: path.join(root, 'calculator-toolbar-filter-search.png') });
    await openFilter(); await page.locator('#formula-filter-options [data-value=""]').click();
    await page.locator('#formula-search-toggle').click();
    await page.locator('#formula-search').fill('query');
    assert.equal(await page.locator('.formula-view-row:visible').count(), 1);
    await page.locator('#formula-search').fill('not here');
    assert.match(await page.locator('#formula-no-matches').innerText(), /No matching/);
    await page.locator('#formula-search').fill('');
    const beforeReloads = await page.evaluate(() => window.__calls.filter(command => command === 'discover_kanban_views').length);
    await page.locator('#formula-reload').click();
    await page.waitForFunction(previous => window.__calls.filter(command => command === 'discover_kanban_views').length > previous, beforeReloads);

    const modal = page.locator('#formula-calculator-dialog');
    const key = label => modal.locator('.formula-keypad').getByRole('button', { name: label, exact: true });
    await page.locator('.formula-view-row').filter({ hasText: 'Tasks' }).click();
    await modal.waitFor({ state: 'visible' });
    await key('Fraction').click(); await key('2').click();
    assert.equal(await modal.locator('#formula-calculator-save').isDisabled(), true);
    assert.equal(await page.evaluate(() => window.BlobUI.FormulaPage.read().definitions.length), 0);
    assert.equal(await page.evaluate(() => window.__calls.includes('evaluate_kanban_formula')), false);
    await modal.locator('#formula-calculator-close').click();
    await modal.waitFor({ state: 'hidden' });
    assert.equal(await page.evaluate(() => window.BlobUI.FormulaPage.read().definitions.length), 0);

    const launchRow = page.locator('.formula-view-row').filter({ hasText: 'Launch plan' });
    await launchRow.click();
    await modal.waitFor({ state: 'visible' });
    assert.equal(await modal.locator('.formula-keypad .formula-key').count(), 25);
    assert.equal(await modal.locator('.formula-calculator-utility .formula-key').count(), 2);
    assert.equal(await modal.locator('#formula-calculator-save').count(), 1);
    assert.equal(await modal.locator('.formula-calculator-actions button').filter({ hasText: 'Save' }).count(), 0);
    assert.equal(await modal.locator('.formula-linear').isVisible(), true);
    assert.equal(await modal.locator('.formula-fraction').isVisible(), false);

    await key('1').click(); await key('2').click(); await key('3').click();
    await modal.getByRole('button', { name: 'Backspace', exact: true }).click();
    await key('4').click();
    assert.equal((await modal.locator('.formula-linear').innerText()).replaceAll('│', '').trim(), '124');
    await key('Change sign').click();
    assert.match((await modal.locator('.formula-linear').innerText()).replace(/[\s│]/g, ''), /−124/);
    await modal.getByRole('button', { name: 'Clear calculation' }).click();

    await modal.getByRole('button', { name: 'Insert property', exact: true }).click();
    await modal.getByLabel('Property', { exact: true }).selectOption('estimate');
    await modal.getByLabel('Aggregation', { exact: true }).selectOption('average');
    await modal.getByRole('button', { name: 'Insert', exact: true }).click();
    assert.match(await modal.locator('.formula-linear').innerText(), /Average Estimate/);
    assert.equal(await page.evaluate(() => window.__calls.includes('evaluate_kanban_formula')), false);

    await key('Fraction').click();
    assert.equal(await modal.locator('.formula-fraction').isVisible(), true);
    assert.equal(await modal.locator('.formula-denominator .formula-placeholder').innerText(), 'y');
    assert.equal(await modal.locator('#formula-calculator-save').isDisabled(), true);
    await key('2').click(); await key('Change sign').click();
    assert.match((await modal.locator('.formula-denominator').innerText()).replace(/[\s│]/g, ''), /−2/);
    assert.equal(await modal.locator('#formula-calculator-save').isEnabled(), true);
    assert.equal(await page.evaluate(() => window.__calls.includes('evaluate_kanban_formula')), false);
    await page.screenshot({ path: path.join(root, 'calculator-modal-dark.png') });
    await page.evaluate(() => document.documentElement.dataset.theme = 'light');
    await page.setViewportSize({ width: 800, height: 740 });
    await page.screenshot({ path: path.join(root, 'calculator-modal-light-narrow.png') });
    assert.equal(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), true);
    await page.evaluate(() => document.documentElement.dataset.theme = 'dark');
    await page.setViewportSize({ width: 1050, height: 1000 });

    await modal.getByRole('button', { name: 'Save calculation' }).click();
    await modal.waitFor({ state: 'hidden' });
    await page.waitForFunction(() => window.__saves.length === 1);
    const saved = await page.evaluate(() => window.__saves[0].definitions[0].tokens);
    assert.deepEqual(saved.map(token => token.kind), ['open', 'aggregate', 'close', 'operator', 'open', 'operator', 'number', 'close']);
    assert.equal(saved[1].property, 'estimate');
    assert.equal(saved[1].operation, 'average');
    assert.equal(saved[5].value, '-');
    assert.equal(saved[6].value, '2');
    assert.match(await page.locator('#formula-calculated-at').innerText(), /Last calculated:/);
    assert.equal(await page.evaluate(() => window.__calls.includes('evaluate_kanban_formula')), false);

    await page.locator('.formula-view-row').filter({ hasText: 'Launch plan' }).click();
    await modal.waitFor({ state: 'visible' });
    assert.match(await modal.locator('.formula-numerator').innerText(), /Average Estimate/);
    assert.match((await modal.locator('.formula-denominator').innerText()).replace(/[\s│]/g, ''), /−2/);
    await modal.locator('.formula-display').focus();
    await page.keyboard.press('ArrowDown');
    assert.equal(await modal.locator('.formula-display').getAttribute('data-active'), 'denominator');
    assert.equal(await modal.locator('.formula-caret').count(), 1);
    assert.equal(await modal.locator('.formula-caret').evaluate(node => node.closest('.formula-denominator') !== null), true);
    await page.keyboard.press('ArrowUp');
    assert.equal(await modal.locator('.formula-display').getAttribute('data-active'), 'numerator');
    await key('Add').click(); await key('1').click();

    await page.evaluate(() => window.__failSave = true);
    await modal.getByRole('button', { name: 'Save calculation' }).click();
    await page.locator('#formula-storage-message').filter({ hasText: /board is unavailable/ }).waitFor();
    assert.equal(await modal.isVisible(), true);
    assert.equal(await page.evaluate(() => window.__saves.length), 1);
    await page.evaluate(() => window.__failSave = false);
    await modal.getByRole('button', { name: 'Save calculation' }).click();
    await modal.waitFor({ state: 'hidden' });
    await page.waitForFunction(() => window.__saves.length === 2);

    await page.evaluate(() => window.__hideProperty = true);
    await page.locator('#formula-reload').click();
    await page.locator('.formula-view-row').filter({ hasText: 'Launch plan' }).click();
    await modal.waitFor({ state: 'visible' });
    await modal.locator('.formula-calculator-status').filter({ hasText: /saved property is unavailable/i }).waitFor();
    await modal.locator('#formula-calculator-close').click();
    await modal.waitFor({ state: 'hidden' });
    await page.evaluate(() => { window.__hideView = true; });
    await page.locator('#formula-reload').click();
    const unavailable = page.locator('.formula-view-row').filter({ hasText: 'Unavailable view' });
    await unavailable.waitFor();
    await unavailable.click();
    await modal.waitFor({ state: 'visible' });
    await modal.getByRole('button', { name: 'Remove calculation' }).click();
    await modal.locator('.formula-calculator-actions').getByRole('button', { name: 'Save', exact: true }).click();
    await modal.waitFor({ state: 'hidden' });
    await page.waitForFunction(() => window.__saves.length === 3 && window.__saves[2].definitions.length === 0);
    assert.equal(await page.evaluate(() => window.__calls.includes('evaluate_kanban_formula')), false);
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
