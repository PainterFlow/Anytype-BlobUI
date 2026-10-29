const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const workspace = path.resolve(__dirname, '../..');
const root = path.join(workspace, '.test-run/calendar-property');
const executablePath = process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';

for (const refreshed of [false, true]) test(`calendar property editing, override flow, Save/Undo and recovery, refreshed=${refreshed}`, async () => {
  fs.mkdirSync(path.join(root, 'temp'), { recursive: true });
  process.env.TEMP = process.env.TMP = path.join(root, 'temp');
  const browser = await chromium.launch({ executablePath, headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  const page = await browser.newPage({ viewport: { width: 1050, height: 1000 }, colorScheme: 'dark' });
  const errors = []; page.on('pageerror', error => errors.push(error.message));
  await page.addInitScript(({ refreshed }) => {
    window.__calls = []; window.__saves = []; window.__savedCalendar = { version: 1, types: [] };
    const target = viewId => ({ spaceId: 'space-a', objectId: 'host', blockId: 'calendar', viewId });
    window.__TAURI__ = { core: { invoke: async (command, args) => {
      window.__calls.push(command);
      if (command === 'get_app_state') return { ready: true, theme: 'dark', onboardingComplete: true, hasSavedApiKey: true, settings: { settingsV21Enabled: refreshed } };
      if (command === 'get_calendar_api_availability') return 'available';
      if (command === 'load_kanban_formulas') return { configuration: { version: 2, display: 'balanced', definitions: [] } };
      if (command === 'discover_kanban_views') return { views: [] };
      if (command === 'load_calendar_rules') return window.__savedCalendar;
      if (command === 'save_calendar_rules') {
        if (window.__failSave) throw Error('API unavailable');
        window.__savedCalendar = JSON.parse(JSON.stringify(args.configuration)); window.__saves.push(args.configuration);
        return { calendarStatus: 'refreshed', info: [] };
      }
      if (command === 'discover_calendar_colors') {
        if (window.__failDiscover) throw Error('Discovery unavailable');
        return { types: [
          { spaceId: 'space-a', key: 'task', name: 'Task', space: 'Work', properties: window.__hideProperty ? [] : [
            { key: 'status', name: 'Status', format: 'select' }, { key: 'done', name: 'Done', format: 'checkbox' }, { key: 'tags', name: 'Tags', format: 'multi_select' }
          ] },
          { spaceId: 'space-b', key: 'task', name: 'Task', space: 'Home', properties: [] },
          { spaceId: 'space-a', key: 'note', name: 'Note', space: 'Work', properties: [] }
        ], views: ['month', 'week'].map(viewId => ({ target: target(viewId), name: viewId === 'month' ? 'Month' : 'Week', space: 'Work', source: 'Tasks', kind: 'Calendar' })) };
      }
      if (command === 'save_settings') return { calendarStatus: 'refreshed' };
      return null;
    } } };
  }, { refreshed });
  const modal = page.locator('#calendar-rule-dialog');
  const typeRow = () => page.locator('#calendar-type-list .formula-view-row').filter({ hasText: 'Task' }).filter({ hasText: 'Work' });
  const choose = async (name, property) => {
    await modal.getByRole('button', { name, exact: false }).first().click();
    await modal.getByRole('option', { name: property, exact: true }).click();
  };
  const close = async () => { await modal.getByRole('button', { name: 'Close color rules' }).click(); await modal.waitFor({ state: 'hidden' }); };
  try {
    await page.goto(pathToFileURL(path.join(workspace, 'settings-app/ui/index.html')).href);
    await page.waitForFunction(() => window.appReady && window.__calls.includes('load_calendar_rules'));
    assert.equal(await page.locator('#open-calendar-colors').count(), 1);
    await page.locator('#open-calendar-colors').evaluate(button => button.click());
    await typeRow().click();
    await choose('Default', 'Status');
    assert.equal(await modal.getByRole('button', { name: /Default Status/ }).count(), 1);
    await modal.getByRole('button', { name: 'Default Status', exact: true }).click();
    assert.deepEqual(await modal.getByRole('option').allTextContents().then(text => text.map(t => t.trim())), ['Use Calendar Color', 'No color', 'DoneCheckbox', 'StatusSingle-select']);
    await modal.getByRole('option', { name: 'Status', exact: true }).click();
    await modal.getByRole('button', { name: 'Add override', exact: true }).click();
    await modal.getByRole('button', { name: /Month/ }).click();
    await choose('Month', 'No color');
    await modal.getByRole('button', { name: 'Add override', exact: true }).click();
    assert.equal(await modal.getByRole('button', { name: /Month/ }).count(), 0, 'Existing calendar overrides cannot be duplicated.');
    await page.keyboard.press('Escape');
    assert.equal(await modal.getByRole('button', { name: 'Add override', exact: true }).count(), 1);
    await modal.screenshot({ path: path.join(root, `rules-${refreshed}.png`) });
    await close();
    const read = () => page.evaluate(() => window.BlobUI.CalendarPage.read());
    assert.equal((await read()).types[0].default.key, 'status');
    assert.equal((await read()).types[0].overrides[0].rule.mode, 'none');
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.__saves.length === 1);
    await typeRow().click(); await choose('Default', 'Done'); await close();
    await page.locator('#discard-button').click();
    await page.locator('#discard-confirmation-undo').click();
    assert.equal((await read()).types[0].default.key, 'status');
    await typeRow().click(); await choose('Default', 'Done'); await close();
    await page.evaluate(() => { window.__failSave = true; });
    await page.locator('#save-button').click();
    await page.waitForFunction(() => document.querySelector('#status-message').textContent.includes('API unavailable'));
    assert.equal((await read()).types[0].default.key, 'done');
    await page.evaluate(() => { window.__failDiscover = true; });
    await page.locator('#calendar-reload').click();
    await page.waitForFunction(() => document.querySelector('#calendar-rules-message').textContent.includes('Discovery unavailable'));
    assert.equal((await read()).types[0].overrides.length, 1);
    await page.evaluate(() => { window.__failDiscover = false; window.__hideProperty = true; });
    await page.locator('#calendar-reload').click(); await typeRow().click();
    await modal.getByRole('button', { name: /Default Unavailable property/ }).click();
    assert.equal(await modal.getByRole('option', { name: 'Unavailable property', exact: true }).count(), 1);
    await modal.getByRole('option', { name: 'No color', exact: true }).click();
    await modal.getByRole('button', { name: 'Remove override', exact: true }).click();
    assert.equal((await read()).types[0].overrides.length, 0);
    await close();
    await page.locator('#calendar-search-toggle').click(); await page.locator('#calendar-search').fill('Note');
    assert.equal(await page.locator('#calendar-type-list .formula-view-row:visible').count(), 1);
    await page.keyboard.press('Escape');
    await page.locator('#calendar-filter').click(); await page.locator('#calendar-filter-options').getByRole('option', { name: 'Home', exact: true }).click();
    assert.equal(await page.locator('#calendar-type-list .formula-view-row:visible').count(), 1);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});

test('calendar CSS scopes overrides and suppression to their exact view while overflow keeps defaults', async () => {
  const css = fs.readFileSync(path.join(root, 'scoped.css'), 'utf8');
  const browser = await chromium.launch({ executablePath, headless: true });
  const page = await browser.newPage({ colorScheme: 'dark' });
  const calendar = (object, block, view) => `<section id="block-${block}"><div data-root-id="${object}"><div id="view-item-${block}-${view}" class="viewItem active"></div></div><div class="viewContent viewCalendar"><div class="record" id="record-event"><span class="name">Event</span></div></div></section>`;
  try {
    await page.setContent(`<style>:root{--color-purple:purple;--color-green:green;--color-lime:green}body{background:#171717}.record{background:rgb(40,40,40);color:rgb(200,200,200)}${css}</style>
      <main id="green">${calendar('host','calendar','green')}</main><main id="none">${calendar('host','calendar','none')}</main>
      <main id="other-view">${calendar('host','calendar','other')}</main><main id="other-object">${calendar('other','calendar','green')}</main><main id="other-block">${calendar('host','other','green')}</main>
      <div class="content"><div class="wrap"><div class="items"><div class="item" id="item-event"><i class="iconObject" id="menuCalendarDay-event-icon"></i><span class="name">Event</span></div></div></div></div>`);
    const border = id => page.locator(`#${id} .record`).evaluate(el => getComputedStyle(el).borderLeftColor);
    assert.equal(await border('green'), 'rgb(0, 128, 0)');
    for (const id of ['other-view','other-object','other-block']) assert.equal(await border(id), 'rgb(128, 0, 128)');
    assert.equal(await page.locator('#none .record').evaluate(el => getComputedStyle(el).backgroundColor), 'rgb(40, 40, 40)');
    assert.equal(await page.locator('#none .name').evaluate(el => getComputedStyle(el).color), 'rgb(200, 200, 200)');
    assert.equal(await page.locator('#item-event').evaluate(el => getComputedStyle(el).borderLeftColor), 'rgb(128, 0, 128)');
  } finally { await browser.close(); }
});

