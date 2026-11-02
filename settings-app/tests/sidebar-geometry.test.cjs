const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const source = path.resolve(__dirname, '../ui');

test('sidebar row geometry stays stable with expandable sections', async () => {
  const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  try {
    const context = await browser.newContext({ viewport: { width: 1000, height: 850 }, colorScheme: 'dark' });
    await context.route('http://fixture/**', route => {
      const file = new URL(route.request().url()).pathname.slice(1) || 'index.html';
      route.fulfill({ contentType: file.endsWith('.css') ? 'text/css' : file.endsWith('.js') ? 'application/javascript' : 'text/html', body: fs.readFileSync(path.join(source, file)) });
    });
    const page = await context.newPage();
    const errors = []; page.on('pageerror', e => errors.push(e.message));
    await page.addInitScript(() => {
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        if (command === 'get_app_state') return { ready: true, onboardingComplete: true, theme: 'dark', settings: JSON.parse(sessionStorage.getItem('saved') || '{}'), hasSavedApiKey: true, apiIp: '127.0.0.1', apiPort: 31009 };
        if (command === 'get_updater_state') return { automaticChecksEnabled: false };
        if (command === 'get_calendar_api_availability') return 'available';
        if (command === 'save_settings') {
          if (window.failSave) throw Error('fixture save failed');
          window.saved = args.settings; sessionStorage.setItem('saved', JSON.stringify(args.settings)); return {};
        }
        if (command === 'configure_connection' || command === 'reveal_api_key') throw Error('Unexpected credential operation');
        return {};
      } } };
    });
    await page.goto('http://fixture/'); await page.waitForFunction(() => window.appReady);


    const geometry = () => page.locator('.nav-item:visible').evaluateAll(nodes => nodes.map(n => ({ width:n.getBoundingClientRect().width, height:n.getBoundingClientRect().height })));
    const baseline = (await geometry())[0];
    await page.evaluate(() => window.BlobUI.SettingsV21.setEnabled(true));
    for (const id of ['preferences', 'advanced', 'codex']) {
      for (const expanded of [true, false]) {
        await page.locator('#'+id+'-nav').click();
        await page.waitForTimeout(300);
        assert.equal(await page.locator('#'+id+'-nav').getAttribute('aria-expanded'), String(expanded));
        for (const row of await geometry()) assert.deepEqual(row, baseline);
        assert.equal(await page.locator('#sidebar-nav').evaluate(n => n.scrollHeight > n.clientHeight), false);
      }
    }
    await page.setViewportSize({ width: 1000, height: 400 });
    await page.locator('#preferences-nav').click();
    await page.waitForTimeout(300);
    assert.equal(await page.locator('#sidebar-nav').evaluate(n => n.scrollHeight > n.clientHeight), true);
    await page.locator('#codex-nav').scrollIntoViewIfNeeded();
    assert.equal(await page.locator('#codex-nav').isVisible(), true);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});
