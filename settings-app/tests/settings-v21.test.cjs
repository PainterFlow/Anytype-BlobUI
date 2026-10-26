const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const source = path.resolve(__dirname, '../ui');

// This suite creates its own browser and a mocked bridge. It never attaches to
// a running Settings instance and cannot change a native API connection.
test('settings experiment uses Docs disclosures, section icons and full-width tabs without search', async () => {
  const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 850 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const html = fs.readFileSync(path.join(source, 'index.html'), 'utf8')
      .replace(/<script[^>]*src="[^"]+"[^>]*><\/script>/g, '')
      .replace('<link rel="stylesheet" href="./styles.css" />', '');
    await page.setContent(html);
    await page.addStyleTag({ path: path.join(source, 'styles.css') });
    await page.addStyleTag({ path: path.join(source, 'formula-page.css') });
    await page.evaluate(() => {
      window.mockCalls = [];
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        window.mockCalls.push(command);
        if (command === 'get_app_state') return { ready: true, onboardingComplete: true, theme: 'dark', settings: {}, hasSavedApiKey: true, apiIp: '127.0.0.1', apiPort: 31009 };
        if (command === 'get_updater_state') return { installDirectory: 'C:/fixture', automaticChecksEnabled: false };
        if (command === 'get_calendar_api_availability') return 'available';
        if (command === 'save_settings') {
          if (window.failSave) throw Error('fixture save failed');
          window.saved = args.settings;
          return {};
        }
        if (command === 'configure_connection' || command === 'reveal_api_key') throw Error('This layout test must never change or reveal a native credential.');
        return {};
      } } };
    });
    for (const name of fs.readFileSync(path.join(source, 'index.html'), 'utf8').matchAll(/<script src="\.\/([^"]+)"/g)) {
      await page.addScriptTag({ path: path.join(source, name[1]) });
    }
    await page.waitForFunction(() => window.appReady);
    await page.locator('#advanced-nav').click();
    await page.locator('#advanced-experiments-tab').click();
    await page.locator('#settings-v21-enabled').check();
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.saved?.settingsV21Enabled === true);
    await page.locator('#settings-v21-enabled').uncheck();
    await page.evaluate(() => window.BlobUI.discardChanges());
    assert.equal(await page.locator('html').getAttribute('data-settings-v21'), 'true');
    await page.locator('#settings-v21-enabled').uncheck();
    await page.evaluate(() => { window.failSave = true; });
    await page.locator('#save-button').click();
    await page.waitForFunction(() => document.getElementById('status-message').textContent.includes('fixture save failed'));
    await page.evaluate(() => { window.failSave = false; window.BlobUI.discardChanges(); });
    assert.equal(await page.locator('input[type="search"]:not(#formula-search), .v21-search').count(), 0);
    assert.equal(await page.locator('#advanced-section-links .v21-section-link').nth(1).innerText(), 'Ap Settings');
    assert.equal(await page.locator('.v21-section-link svg').count(), 8);
    assert.equal(await page.locator('.v21-section-links .documentation-chevron').count(), 0);
    await page.locator('#preferences-nav').click();
    assert.equal(await page.locator('#preferences-nav').getAttribute('aria-expanded'), 'true');
    await page.locator('#preferences-nav').click();
    await page.locator('#preferences-section-links').waitFor({ state: 'hidden' });
    await page.locator('#preferences-nav').click();
    const disclosureStyle = await page.locator('#preferences-section-links').evaluate(node => ({
      duration: getComputedStyle(node).transitionDuration,
      easing: getComputedStyle(node).transitionTimingFunction,
    }));
    assert.ok(disclosureStyle.duration.split(',').every(value => value.trim() === '0.2s'));
    assert.match(disclosureStyle.easing, /cubic-bezier/);
    const widths = await page.locator('.preference-tabs [role="tab"]').evaluateAll(nodes => nodes.map(node => node.getBoundingClientRect().width));
    assert.ok(Math.max(...widths) - Math.min(...widths) < 1, 'Preferences tabs divide the width equally');
    await page.locator('#preference-tab-0').focus(); await page.keyboard.press('End');
    assert.equal(await page.locator('#preference-tab-4').getAttribute('aria-selected'), 'true');
    await page.keyboard.press('Home');
    assert.equal(await page.locator('#preference-tab-0').getAttribute('aria-selected'), 'true');
    await page.locator('#preference-tab-3').click(); await page.locator('#font-family').fill('Arial');
    await page.locator('#preferences-section-links').getByRole('button', { name: 'Calendar', exact: true }).click();
    assert.match(await page.locator('#calendar-colors-help').innerText(), /Alt\+R.*Ctrl\+R/);
    await page.locator('#accessibility-nav').click();
    assert.equal(await page.locator('#accessibility-nav').getAttribute('aria-expanded'), null);
    assert.equal(await page.locator('#accessibility-section-links').count(), 0);
    assert.equal(await page.locator('#accessibility-page > section').nth(1).evaluate(node => getComputedStyle(node).borderTopWidth), '0px');
    await page.locator('#advanced-nav').click(); await page.locator('#advanced-app-tab').click();
    const advancedWidths = await page.locator('#advanced-page [role="tab"]').evaluateAll(nodes => nodes.map(node => node.getBoundingClientRect().width));
    assert.ok(Math.max(...advancedWidths) - Math.min(...advancedWidths) < 1, 'Advanced tabs divide the width equally');
    assert.equal(await page.locator('#advanced-app-panel > section').nth(1).evaluate(node => getComputedStyle(node).borderTopWidth), '0px');
    await page.locator('#advanced-api-tab').click();
    await page.locator('#advanced-api-key').fill('fixture-pending-key');
    await page.locator('#advanced-experiments-tab').click(); await page.locator('#settings-v21-enabled').uncheck();
    await page.locator('#preferences-nav').click();
    assert.equal(await page.locator('#font-family').inputValue(), 'Arial');
    assert.equal(await page.locator('#advanced-api-key').inputValue(), 'fixture-pending-key');
    assert.equal(await page.locator('.preference-tabs').isVisible(), false);
    await page.locator('#advanced-nav').click(); await page.locator('#advanced-experiments-tab').click();
    await page.locator('#settings-v21-enabled').check();
    await page.locator('#preferences-nav').click(); await page.locator('#preference-tab-0').click();
    await page.waitForFunction(() => {
      const panel = document.getElementById('preferences-section-links');
      return panel.getBoundingClientRect().height >= panel.firstElementChild.scrollHeight;
    });
    for (const theme of ['dark', 'light']) {
      await page.evaluate(theme => { document.documentElement.dataset.theme = theme; }, theme);
      await page.screenshot({ path: path.resolve(__dirname, `../../.test-run/settings-v21-revised-${theme}.png`) });
    }
    await page.setViewportSize({ width: 800, height: 650 }); await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.locator('#preferences-nav').click(); await page.locator('#preferences-nav').click();
    assert.equal(await page.locator('#preferences-nav').getAttribute('aria-expanded'), 'true');
    await page.screenshot({ path: path.resolve(__dirname, '../../.test-run/settings-v21-revised-narrow.png') });
    assert.equal(await page.evaluate(() => window.mockCalls.includes('configure_connection')), false);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});
