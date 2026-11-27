const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const { loadSettings } = require('./frontend-fixture.cjs');

function settingsLinks() {
  const context = { console, URL };
  vm.createContext(context);
  vm.runInContext(fs.readFileSync(path.resolve(__dirname, '../ui/settings-links.js'), 'utf8'), context);
  return context.BlobUI.SettingsLinks;
}

test('settings page links parse visible tab names and reject incomplete targets', () => {
  const links = settingsLinks();
  assert.deepEqual({ ...links.parse('blob://settings.page/preferences/colors#accent-color') }, {
    page: 'preferences', child: 'colors', childIndex: 0, section: 'accent-color',
  });
  assert.deepEqual({ ...links.parse('blob://settings.page/advanced-settings/app-settings#backup-and-import') }, {
    page: 'advanced', child: 'app-settings', childIndex: 1, section: 'backup-and-import',
  });
  assert.deepEqual({ ...links.parse('blob://settings.page/anyblob/settings#object-mentions') }, {
    page: 'codex', child: 'settings', childIndex: 0, section: 'object-mentions',
  });
  assert.deepEqual({ ...links.parse('blob://settings.page/accessibility#hover-effects') }, {
    page: 'accessibility', child: '', childIndex: null, section: 'hover-effects',
  });
  for (const [href, page, child, childIndex] of [
    ['blob://settings.page/preferences/font', 'preferences', 'font', 1],
    ['blob://settings.page/preferences/space-banner', 'preferences', 'space-banner', 2],
    ['blob://settings.page/calendar-events/appearance', 'calendar-events', 'appearance', 0],
    ['blob://settings.page/calendar-events/color-by-property', 'calendar-events', 'color-by-property', 1],
    ['blob://settings.page/kanban-views/appearance', 'kanban-views', 'appearance', 0],
    ['blob://settings.page/kanban-views/kanban-calculations', 'kanban-views', 'kanban-calculations', 1],
    ['blob://settings.page/advanced/api-key', 'advanced', 'api-key', 0],
    ['blob://settings.page/advanced/experiments', 'advanced', 'experiments', 2],
    ['blob://settings.page/codex/permissions', 'codex', 'permissions', 1],
  ]) {
    assert.deepEqual({ ...links.parse(href) }, { page, child, childIndex, section: '' });
  }
  assert.equal(links.parse('blob://settings.page/preferences#accent-color'), null);
  assert.equal(links.parse('blob://settings.page/accessibility/reduced-motion#reduced-motion'), null);
  assert.equal(links.parse('blob://settings.page/unknown/colors#accent-color'), null);
  assert.equal(links.parse('blob://settings.page/preferences/colors?x=1#accent-color'), null);
  assert.equal(links.parse('https://settings.page/preferences/colors#accent-color'), null);
});

test('settings page links render in Markdown, close the current dialog, and navigate from any surface', async () => {
  const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  try {
    const page = await browser.newPage({ viewport: { width: 800, height: 650 }, colorScheme: 'dark' });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(() => {
      window.__TAURI__ = {
        event: { listen: async () => () => {} },
        core: { invoke: async command => {
          if (command === 'get_app_state') return { ready: true, onboardingComplete: true, theme: 'dark', settings: {}, hasSavedApiKey: true };
          if (command === 'get_updater_state') return { installDirectory: 'fixture', automaticChecksEnabled: false };
          if (command === 'get_calendar_api_availability') return 'available';
          if (command === 'anyblob_status') return { enabled: false, agentsGuide: false };
          if (command === 'configure_connection' || command === 'reveal_api_key') throw Error('Credential operations are forbidden in settings-link tests');
          return {};
        } },
      };
    });
    await loadSettings(page);
    await page.waitForFunction(() => window.appReady);

    await page.locator('#help-button').click();
    await page.locator('#help-dev-guide').click();
    await page.locator('#documentation-dialog').waitFor({ state: 'visible' });
    await page.evaluate(() => {
      const host = document.createElement('div');
      host.id = 'fixture-settings-link-host';
      host.className = 'markdown-content';
      const dialog = document.getElementById('documentation-dialog');
      dialog.insertBefore(host, dialog.children[1] || null);
      window.BlobUI.Markdown.mount(host, '[Open backup settings](blob://settings.page/advanced/app-settings#backup-and-import)');
    });
    assert.equal(await page.locator('#fixture-settings-link-host a').count(), 1, 'The Markdown sanitizer keeps a valid settings link.');
    await page.locator('#fixture-settings-link-host a').click();
    await page.waitForFunction(() => document.getElementById('documentation-backdrop').hidden);
    await page.waitForFunction(() => document.activeElement?.id === 'backup-heading');
    assert.equal(await page.locator('#page-title').innerText(), 'Advanced Settings');
    assert.equal(await page.locator('#advanced-app-tab').getAttribute('aria-selected'), 'true');
    assert.ok(await page.locator('.page-content').evaluate(scroller => scroller.scrollTop) > 0, 'The section fragment scrolls the target heading to the top.');

    await page.evaluate(() => {
      const link = document.createElement('a');
      link.id = 'fixture-accessibility-link';
      link.href = 'blob://settings.page/accessibility#hover-effects';
      link.textContent = 'Open hover effects';
      document.querySelector('.settings-content').append(link);
    });
    await page.locator('#fixture-accessibility-link').click();
    await page.waitForFunction(() => document.activeElement?.id === 'hover-effects-heading');
    assert.equal(await page.locator('#page-title').innerText(), 'Accessibility');
    assert.equal(await page.locator('#accessibility-page').isVisible(), true);

    await page.evaluate(() => {
      const link = document.createElement('a');
      link.id = 'fixture-anyblob-link';
      link.href = 'blob://settings.page/anyblob/settings#object-mentions';
      link.textContent = 'Open object mentions';
      document.querySelector('.settings-content').append(link);
    });
    await page.locator('#fixture-anyblob-link').click();
    await page.waitForFunction(() => document.activeElement?.textContent === 'Object Mentions');
    assert.equal(await page.locator('#page-title').innerText(), 'Anyblob');
    assert.equal(await page.locator('#codex-tab-0').getAttribute('aria-selected'), 'true');
    assert.equal(await page.locator('#codex-panel-0').isVisible(), true);
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
