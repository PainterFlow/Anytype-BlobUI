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
        ], views: (window.__manyViews ? Array.from({length:60}, (_,index) => 'view-' + index) : ['month', 'week']).map(viewId => ({ target: target(viewId), name: viewId === 'month' ? 'Month' : 'Week', space: 'Work', source: 'Tasks', kind: 'Calendar' })) };
      }
      if (command === 'save_settings') return { calendarStatus: 'refreshed' };
      return null;
    } } };
  }, { refreshed });
  const modal = page.locator('#calendar-rule-dialog');
  const typeRow = () => page.locator('#calendar-type-list .formula-view-row').filter({ hasText: 'Task' }).filter({ hasText: 'Work' });
  const choose = async (name, property) => {
    await modal.getByRole('button', { name: name + ' color property', exact: true }).click();
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
    assert.equal(await modal.getByRole('button', { name: 'Default color property', exact: true }).count(), 1);
    assert.equal(await modal.locator('.calendar-property-picker .formula-insert-property').count(), 1);
    assert.equal(await modal.locator('.calendar-picker-chevron').count(), 0);
    await modal.getByRole('button', { name: 'Default color property', exact: true }).click();
    assert.deepEqual(await modal.getByRole('option').allTextContents().then(text => text.map(t => t.trim())), ['Use Calendar Color', 'No color', 'Done', 'Status']);
    await modal.getByRole('option', { name: 'Status', exact: true }).click();
    await modal.getByRole('button', { name: 'Add override', exact: true }).click();
    await modal.getByRole('button', { name: /Month/ }).click();
    await choose('Month', 'No color');
    await modal.getByRole('button', { name: 'Add override', exact: true }).click();
    assert.equal(await modal.getByRole('button', { name: /Month/ }).count(), 0, 'Existing calendar overrides cannot be duplicated.');
    await page.keyboard.press('Escape');
    await modal.getByRole('button', { name: 'Add override', exact: true }).waitFor({ state: 'visible' });
    assert.equal(await modal.getByRole('button', { name: 'Add override', exact: true }).count(), 1);
    assert.equal(await modal.getByText("Select uses Anytype's option color.", { exact: false }).count(), 0);
    assert.equal(await modal.locator('.calendar-property-picker .kanban-select-menu').count(), 0, 'Menus overlay the dialog instead of creating an embedded scrolling panel.');
    await modal.getByRole('button', { name: 'Default color property', exact: true }).focus();
    assert.equal(await modal.getByRole('button', { name: 'Default color property', exact: true }).evaluate(el => getComputedStyle(el).outlineStyle), 'none');
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
    await modal.getByRole('button', { name: 'Default color property', exact: true }).click();
    assert.equal(await modal.getByRole('option', { name: 'Unavailable property', exact: true }).count(), 1);
    await modal.getByRole('option', { name: 'No color', exact: true }).click();
    const remove = modal.getByRole('button', { name: 'Remove override', exact: true });
    assert.equal(await remove.textContent(), '');
    assert.equal(await remove.locator('svg').count(), 1);
    assert.equal(await remove.evaluate(el => el.closest('.calendar-rule-name-group') !== null), true);
    assert.equal(await remove.evaluate(el => { const values = getComputedStyle(el).color.match(/\d+/g).map(Number); return values[0] > values[1] && values[0] > values[2]; }), true);
    await remove.click();
    assert.equal((await read()).types[0].overrides.length, 0);
    await close();
    await page.evaluate(() => { window.__manyViews = true; });
    await page.locator('#calendar-reload').click(); await typeRow().click();
    await modal.getByRole('button', { name: 'Add override', exact: true }).click();
    await modal.evaluate(el => Promise.all(el.getAnimations().map(animation => animation.finished)));
    const top = await modal.locator('.calendar-modal-header').evaluate(el => el.getBoundingClientRect().top);
    const toolbarTop = await modal.locator('.calendar-sticky-toolbar').evaluate(el => el.getBoundingClientRect().top);
    await modal.locator('.calendar-modal-content').evaluate(el => { el.scrollTop = el.scrollHeight; el.dispatchEvent(new Event('scroll')); });
    assert.equal(await modal.locator('.calendar-modal-header').evaluate(el => el.getBoundingClientRect().top), top);
    assert.equal(await modal.locator('.calendar-sticky-toolbar').evaluate(el => el.getBoundingClientRect().top), toolbarTop);
    assert.equal(await modal.evaluate(el => el.scrollHeight <= el.clientHeight), true);
    await modal.screenshot({ path: path.join(root, 'calendar-chooser-' + refreshed + '.png') });
    assert.equal(await page.evaluate(() => {
      document.querySelector('#calendar-rule-dialog [aria-label="Close color rules"]').click();
      return document.querySelector('#calendar-rule-backdrop').classList.contains('is-closing') && !document.querySelector('#calendar-rule-backdrop').hidden;
    }), true, 'Close animates from the calendar chooser too.');
    await modal.getByRole('button', { name: 'Default color property', exact: true }).waitFor({ state: 'visible' });
    assert.equal(await modal.getByRole('button', { name: 'Back', exact: true }).count(), 0);
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

test('single-select array values override legacy colors through the API snapshot, compiler, and calendar DOM', async () => {
  const css = fs.readFileSync(path.join(root, 'array-priority.css'), 'utf8');
  const browser = await chromium.launch({ executablePath, headless: true });
  const page = await browser.newPage({ colorScheme: 'dark' });
  try {
    await page.setContent(`<style>${css}
      :root{--color-blue:#0011ff!important;--color-purple:#800080!important}
      body{background:#171717}.record{background:#282828;border-left:3px solid pink}
      </style><div class="viewContent viewCalendar"><div class="record" id="record-event">Array selection</div><div class="record" id="record-empty">Empty selection</div></div>
      <div data-view-type="4" data-drop-type="record" data-id="event">Drag record</div>`);
    for (const selector of ['#record-event', '[data-view-type="4"][data-id="event"]']) {
      assert.equal(await page.locator(selector).evaluate(el => getComputedStyle(el).borderLeftColor), 'rgb(0, 17, 255)');
    }
    assert.equal(await page.locator('#record-empty').evaluate(el => getComputedStyle(el).borderLeftColor), 'rgb(128, 0, 128)');
  } finally { await browser.close(); }
});

test('saved calendar rules do not become unavailable during initial loading, failed discovery, or reload', async () => {
  fs.mkdirSync(path.join(root, 'temp'), { recursive: true });
  process.env.TEMP = process.env.TMP = path.join(root, 'temp');
  const browser = await chromium.launch({ executablePath, headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  const page = await browser.newPage({ viewport: { width: 900, height: 800 }, colorScheme: 'dark' });
  await page.addInitScript(() => {
    window.__space = 'bafy' + 'x'.repeat(60) + '.replication-key';
    window.__savedRules = { version: 1, types: [{ spaceId: window.__space, typeKey: 'task', default: { mode: 'property', key: 'status' }, overrides: [] }] };
    window.__TAURI__ = { core: { invoke: async command => {
      if (command === 'get_app_state') return { ready: true, theme: 'dark', onboardingComplete: true, hasSavedApiKey: true, settings: {} };
      if (command === 'get_calendar_api_availability') return 'available';
      if (command === 'load_kanban_formulas') return { configuration: { version: 2, display: 'balanced', definitions: [] } };
      if (command === 'discover_kanban_views') return { views: [] };
      if (command === 'load_calendar_rules') return window.__savedRules;
      if (command === 'discover_calendar_colors') return new Promise((resolve, reject) => { window.__pendingDiscover = { resolve, reject }; });
      return null;
    } } };
  });
  const resolveCatalog = async missing => page.evaluate(missing => {
    const pending = window.__pendingDiscover; window.__pendingDiscover = null;
    pending.resolve({ types: missing ? [] : [{ spaceId: window.__space, key: 'task', name: 'Task', space: 'Work', properties: [] }], views: [] });
  }, missing);
  const rejectCatalog = async () => page.evaluate(() => { const pending = window.__pendingDiscover; window.__pendingDiscover = null; pending.reject(Error('Fixture discovery unavailable')); });
  const list = page.locator('#calendar-type-list');
  try {
    await page.goto(pathToFileURL(path.join(workspace, 'settings-app/ui/index.html')).href);
    await page.waitForFunction(() => window.appReady && !document.querySelector('#open-calendar-colors').disabled);
    await page.locator('#open-calendar-colors').evaluate(button => button.click());
    await page.waitForFunction(() => !!window.__pendingDiscover);
    assert.match(await list.textContent(), /Loading object types/);
    assert.doesNotMatch(await list.textContent(), /Unavailable type/);
    await rejectCatalog();
    await page.waitForFunction(() => document.querySelector('#calendar-rules-message').textContent.includes('Fixture discovery unavailable'));
    assert.doesNotMatch(await list.textContent(), /Unavailable type/);
    assert.equal(await list.getByRole('button', { name: 'Remove saved rules' }).count(), 0);
    await page.locator('#calendar-reload').click(); await page.waitForFunction(() => !!window.__pendingDiscover);
    await resolveCatalog(false); await list.getByRole('button', { name: 'Task Work' }).waitFor({ state: 'visible' });
    await page.evaluate(() => { void window.BlobUI.CalendarPage.reload(); });
    await page.waitForFunction(() => !!window.__pendingDiscover);
    assert.match(await list.textContent(), /Task/);
    assert.doesNotMatch(await list.textContent(), /Unavailable type/);
    await rejectCatalog();
    await page.waitForFunction(() => !document.querySelector('#calendar-reload').disabled);
    assert.match(await list.textContent(), /Task/);
    await page.locator('#calendar-reload').click(); await page.waitForFunction(() => !!window.__pendingDiscover);
    await resolveCatalog(true);
    const row = list.locator('.formula-view-row');
    await row.filter({ hasText: 'Unavailable type' }).waitFor({ state: 'visible' });
    const layout = await row.evaluate(el => {
      const title = el.querySelector('.formula-view-name'), warning = el.querySelector('.formula-view-warning');
      return { width: el.clientWidth, titleWidth: title.getBoundingClientRect().width, warningWidth: warning.getBoundingClientRect().width, overflowing: el.scrollWidth > el.clientWidth };
    });
    assert.ok(layout.titleWidth > layout.width * 0.35);
    assert.ok(layout.warningWidth > layout.width * 0.8);
    assert.equal(layout.overflowing, false);
    assert.deepEqual(await page.evaluate(() => window.BlobUI.CalendarPage.read()), await page.evaluate(() => window.__savedRules));
    await list.screenshot({ path: path.join(root, 'unavailable-row-dark.png') });
  } finally { await browser.close(); }
});

