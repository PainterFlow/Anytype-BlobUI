const test = require('node:test');
const assert = require('node:assert/strict');
const { loadSettings } = require('./frontend-fixture.cjs');
const path = require('node:path');
const source = path.resolve(__dirname, '../ui');

test('sidebar resizing shares geometry, persists, cancels and preserves navigation in the standard layout', async () => {
  const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  try {
    const context = await browser.newContext({ viewport: { width: 1000, height: 850 }, colorScheme: 'dark' });
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
    await loadSettings(page); await page.waitForFunction(() => window.appReady);
    const handle = page.locator('.sidebar > .sidebar-resizer');
    const width = () => page.locator('.sidebar').evaluate(n => n.getBoundingClientRect().width);
    assert.equal(await page.locator('#sidebar-resize-mode').count(), 0);
    assert.equal(await handle.isVisible(), true);
    assert.equal(await width(), 256);
    await handle.focus(); await page.keyboard.press('Home'); assert.equal(await width(), 160);
    await page.locator('#advanced-nav').hover();
    assert.equal(await page.locator('#advanced-nav').getAttribute('title'), 'Advanced Settings');
    assert.equal(await page.locator('#sidebar-navigation-tooltip').isVisible(), false);
    await handle.focus(); await page.keyboard.press('End'); assert.equal(await width(), 320);
    assert.equal(await page.locator('#advanced-nav').getAttribute('title'), null);
    assert.equal(await page.locator('#save-button').isVisible(), false);
    await page.reload(); await page.waitForFunction(() => window.appReady); assert.equal(await width(), 320);
    const box = await handle.boundingBox();
    await page.mouse.move(box.x + 4, box.y + 50); await page.mouse.down(); await page.mouse.move(220, box.y + 50);
    await page.keyboard.press('Escape'); await page.mouse.up(); assert.equal(await width(), 320);
    await handle.focus(); await page.keyboard.press('Home'); assert.equal(await width(), 160);
    await page.keyboard.press('ArrowLeft'); assert.equal(await width(), 160);
    await page.keyboard.press('ArrowRight'); assert.equal(await width(), 168);
    await page.setViewportSize({ width: 600, height: 850 });
    await handle.focus(); await page.keyboard.press('End'); assert.equal(await width(), 240);
    for (const legacy of ['off', 'pro']) {
      await page.evaluate(value => sessionStorage.setItem('saved', JSON.stringify({ sidebarResizeMode: value })), legacy);
      await page.reload(); await page.waitForFunction(() => window.appReady);
      assert.equal(await handle.isVisible(), true); assert.equal(await width(), 240);
      assert.equal(await page.locator('html').getAttribute('data-sidebar-collapsed'), 'false');
    }
    assert.deepEqual(errors, []);
    await context.close();
  } finally { await browser.close(); }
});

test('corrupt or unavailable geometry storage keeps resizing usable', async () => {
  const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true });
  try {
    for (const stored of ['bad json', '{"width":9999,"collapsed":true}', '{"width":"bad"}', null]) {
      const page = await browser.newPage({ viewport: { width: 1000, height: 850 }, colorScheme: 'dark' });
      await page.setContent('<aside class="sidebar"></aside><div class="documentation-layout"></div>');
      await page.addStyleTag({ path: path.join(source, 'styles.css') });
      await page.evaluate(value => Object.defineProperty(window, 'localStorage', { value: value === null ? { getItem() { throw Error('blocked'); }, setItem() { throw Error('blocked'); } } : { getItem() { return value; }, setItem() {} } }), stored);
      await page.addScriptTag({ path: path.join(source, 'sidebar-resize.js') });
      await page.evaluate(() => window.BlobUI.SidebarResize.setMode('standard'));
      assert.equal(await page.locator('.sidebar-resizer').first().getAttribute('aria-valuenow'), stored?.includes('9999') ? '320' : '256');
      if (stored?.includes('9999')) {
        await page.evaluate(() => window.BlobUI.SidebarResize.setMode('pro'));
        assert.equal(await page.locator('.sidebar-resizer').first().getAttribute('aria-valuenow'), '320');
        await page.evaluate(() => window.BlobUI.SidebarResize.setMode('standard'));
      }
      const handle = page.locator('.sidebar-resizer').first(); await handle.focus(); await page.keyboard.press('Home');
      assert.equal(await handle.getAttribute('aria-valuenow'), '160');
      await page.close();
    }
  } finally { await browser.close(); }
});
