const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const { loadSettings } = require('./frontend-fixture.cjs');
const source = path.resolve(__dirname, '../ui');

// This suite creates its own browser and a mocked bridge. It never attaches to
// a running Settings instance and cannot change a native API connection.
test('standard settings uses Docs disclosures, section icons and full-width tabs without search', async () => {
  const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 850 } });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(() => {
      window.mockCalls = [];
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        window.mockCalls.push(command);
        if (command === 'get_app_state') return { ready: true, onboardingComplete: true, theme: 'dark', settings: { settingsV21Enabled: false }, hasSavedApiKey: true, apiIp: '127.0.0.1', apiPort: 31009 };
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
    await loadSettings(page);
    await page.waitForFunction(() => window.appReady);
    assert.equal(await page.locator('#settings-v21-enabled').count(), 0);
    assert.equal(await page.locator('#advanced-experiments-tab').count(), 1);
    assert.equal(await page.locator('.preference-tabs').isVisible(), true);
    assert.equal(await page.locator('#save-button').isVisible(), false);
    assert.equal(await page.locator('#preferences-page input[type="search"], #advanced-page input[type="search"], #accessibility-page input[type="search"], .v21-search').count(), 0);
    assert.equal(await page.locator('#advanced-section-links .v21-section-link').nth(1).innerText(), 'App Settings');
    assert.equal(await page.locator('.v21-section-link svg').count(), 11);
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
    assert.equal(await page.locator('#calendar-colors-help').isHidden(), true);
    await page.locator('#accessibility-nav').click();
    assert.equal(await page.locator('#accessibility-nav').getAttribute('aria-expanded'), null);
    assert.equal(await page.locator('#accessibility-section-links').count(), 0);
    assert.equal(await page.locator('#accessibility-page > section').nth(1).evaluate(node => getComputedStyle(node).borderTopWidth), '0px');
    await page.locator('#advanced-nav').click(); await page.locator('#advanced-app-tab').click();
    const advancedWidths = await page.locator('#advanced-page [role="tab"]').evaluateAll(nodes => nodes.map(node => node.getBoundingClientRect().width));
    assert.ok(Math.max(...advancedWidths) - Math.min(...advancedWidths) < 1, 'Advanced tabs divide the width equally');
    assert.equal(await page.locator('#advanced-app-panel > section').nth(1).evaluate(node => getComputedStyle(node).borderTopWidth), '0px');
    assert.equal(await page.locator('.app-general-settings .settings-list').count(), 0);
    assert.equal(await page.locator('#advanced-app-panel #start-on-login').isVisible(), true);
    assert.equal(await page.locator('#startup-mode, #experiment-run-background').count(), 0);
    assert.equal(await page.locator('.app-backup-box #import-blob-backup-button').count(), 1);
    assert.equal(await page.locator('.app-backup-box #create-blob-backup-button').count(), 1);
    await page.locator('#advanced-api-tab').click();
    await page.locator('#advanced-section-links').getByRole('button', { name: 'Experiments', exact: true }).click();
    assert.equal(await page.locator('#advanced-experiments-tab').getAttribute('aria-selected'), 'true');
    assert.equal(await page.locator('#advanced-experiments-panel').isVisible(), true);
    await page.locator('#advanced-api-tab').click();
    await page.locator('#advanced-api-key').fill('fixture-pending-key');
    await page.locator('#advanced-api-tab').focus();
    await page.keyboard.press('End');
    assert.equal(await page.locator('#advanced-experiments-tab').getAttribute('aria-selected'), 'true');
    assert.equal(await page.locator('#advanced-experiments-panel').isVisible(), true);
    assert.equal(await page.locator('#advanced-experiments-panel #startup-mode, #advanced-experiments-panel #start-on-login, #experiment-run-background').count(), 0);
    assert.equal(await page.locator('#advanced-experiments-panel #background-experiment').isVisible(), true);
    assert.equal(await page.getByRole('radiogroup').count(), 0);
    await page.setViewportSize({ width: 800, height: 650 });
    await page.locator('#background-experiment').focus();
    await page.keyboard.press('Enter');
    for (const [label, prefix] of [['Disabled', 'Use standard Settings controls.'], ['Variant 1', 'Normal.'], ['Variant 2', 'Autostart Background.']]) {
      const option = page.getByRole('option', { name: label, exact: true });
      assert.equal(await option.isVisible(), true);
      const geometry = await option.evaluate(button => {
        const copy = button.querySelector('.kanban-select-option-description');
        const title = button.querySelector('.kanban-option-label');
        const rect = button.getBoundingClientRect();
        const hit = document.elementFromPoint(rect.left + rect.width / 2, rect.top + rect.height / 2);
        return { text: copy.textContent, below: copy.getBoundingClientRect().top >= title.getBoundingClientRect().bottom,
          color: getComputedStyle(copy).color, titleColor: getComputedStyle(title).color, size: parseFloat(getComputedStyle(copy).fontSize), titleSize: parseFloat(getComputedStyle(title).fontSize), overflow: button.scrollWidth > button.clientWidth, reachable: Boolean(hit && button.contains(hit)) };
      });
      assert.ok(geometry.text.startsWith(prefix));
      assert.equal(geometry.below, true);
      assert.notEqual(geometry.color, geometry.titleColor);
      assert.ok(geometry.size < geometry.titleSize);
      assert.equal(geometry.overflow, false);
      assert.equal(geometry.reachable, true, 'Each dropdown option must remain visible outside the settings group.');
    }
    assert.equal(await page.locator('#advanced-experiments-panel').evaluate(node => node.scrollWidth <= node.clientWidth), true);
    await page.screenshot({ path: path.resolve(__dirname, '../../.test-run/advanced-experiments-options-dark.png') });
    await page.keyboard.press('Escape');
    await page.locator('#advanced-experiments-tab').focus();
    await page.waitForFunction(() => {
      const strip = document.getElementById('advanced-tabs');
      const current = new DOMMatrixReadOnly(getComputedStyle(strip, '::after').transform).m41;
      return Math.abs(current - document.getElementById('advanced-experiments-tab').offsetLeft) < 1;
    });
    await page.screenshot({ path: path.resolve(__dirname, '../../.test-run/advanced-experiments-dark.png') });
    await page.keyboard.press('Home');
    assert.equal(await page.locator('#advanced-api-tab').getAttribute('aria-selected'), 'true');
    assert.equal(await page.locator('#advanced-api-key').inputValue(), 'fixture-pending-key');
    await page.locator('#preferences-nav').click();
    await page.locator('#preference-tab-3').click();
    assert.equal(await page.locator('#font-family').inputValue(), 'Arial');
    assert.equal(await page.locator('#advanced-api-key').inputValue(), 'fixture-pending-key');
    assert.equal(await page.locator('.preference-tabs').isVisible(), true);
    await page.locator('#preference-tab-0').click();
    await page.waitForFunction(() => {
      const panel = document.getElementById('preferences-section-links');
      return panel.getBoundingClientRect().height >= panel.firstElementChild.scrollHeight;
    });
    for (const theme of ['dark']) {
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
