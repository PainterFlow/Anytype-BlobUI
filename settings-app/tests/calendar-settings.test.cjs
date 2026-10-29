const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const test = require('node:test');
let chromium;
try { chromium = require(process.env.PLAYWRIGHT_MODULE || 'playwright').chromium; } catch {}
const workspace = path.resolve(__dirname, '../..');
const chrome = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
test('Calendar Colors preserves drafts, gates interaction, and recovers API availability', {
  skip: (!chromium || !fs.existsSync(chrome)) && 'Playwright and Chrome required',
}, async () => {
  const root = path.join(workspace, '.test-run/calendar-settings');
  fs.mkdirSync(path.join(root, 'temp'), { recursive: true });
  process.env.TEMP = process.env.TMP = path.join(root, 'temp');
  const context = await chromium.launchPersistentContext(path.join(root, 'profile'), { headless: true, executablePath: chrome });
  try {
    const page = await context.newPage();
    await page.addInitScript(() => {
      window.__availability = 'missing';
      window.__pending = [];
      window.__saves = [];
      window.__failSave = false;
      window.__TAURI__ = { core: { invoke: async (name, args) => {
        if (name === 'get_app_state') return { ready: true, theme: 'dark', onboardingComplete: true, settings: {} };
        if (name === 'get_calendar_api_availability') {
          if (window.__availability === 'pending') return new Promise(resolve => window.__pending.push(resolve));
          return window.__availability;
        }
        if (name === 'save_settings') {
          if (window.__failSave) throw new Error('Save fixture failed');
          window.__saves.push(args.settings);
          return { calendarStatus: args.settings.calendarColorsEnabled ? 'cached' : 'off' };
        }
        if (name === 'get_theme') return 'dark';
        return null;
      } } };
    });
    await page.goto(pathToFileURL(path.join(workspace, 'settings-app/ui/index.html')).href);
    await page.waitForFunction(() => window.appReady);
    const toggle = page.locator('#calendar-colors-enabled');
    const navigate = async status => {
      await page.evaluate(value => { window.__availability = value; }, status);
      await page.locator('#advanced-nav').click();
      await page.locator('#preferences-nav').click();
    };
    assert.equal(await toggle.isChecked(), true);
    await page.waitForFunction(() => document.getElementById('calendar-colors-help').textContent.includes('Add an Anytype API key'));
    assert.equal(await toggle.isDisabled(), true);
    await navigate('available');
    await toggle.waitFor({ state: 'visible' });
    await page.waitForFunction(() => !document.getElementById('calendar-colors-enabled').disabled);
    await toggle.focus();
    await page.keyboard.press('Space');
    assert.equal(await toggle.isChecked(), false);
    await page.evaluate(() => window.BlobUI.discardChanges());
    assert.equal(await toggle.isChecked(), true);
    await toggle.uncheck();
    await page.evaluate(() => { window.__failSave = true; });
    await page.locator('#save-button').click();
    await page.waitForFunction(() => !document.getElementById('save-button').disabled);
    assert.equal(await toggle.isChecked(), false);
    await page.evaluate(() => { window.__failSave = false; });
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.__saves.length === 1);
    assert.equal(await page.evaluate(() => window.__saves[0].calendarColorsEnabled), false);
    await navigate('unavailable');
    await page.waitForFunction(() => document.getElementById('calendar-colors-help').textContent.includes('unavailable'));
    assert.equal(await toggle.isDisabled(), true);
    assert.equal(await toggle.isChecked(), false);
    await navigate('pending');
    await page.waitForFunction(() => window.__pending.length > 0);
    assert.equal(await toggle.isDisabled(), true);
    await navigate('available');
    await page.waitForFunction(() => !document.getElementById('calendar-colors-enabled').disabled);
    await page.evaluate(() => { window.__pending.splice(0).forEach(resolve => resolve('unavailable')); });
    assert.equal(await toggle.isDisabled(), false);
    assert.equal(await toggle.isChecked(), false);
    for (const theme of ['dark']) {
      await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
      await page.locator('section[aria-labelledby="calendar-heading"]').screenshot({ path: path.join(root, theme + '.png') });
    }
  } finally { await context.close(); }
});
