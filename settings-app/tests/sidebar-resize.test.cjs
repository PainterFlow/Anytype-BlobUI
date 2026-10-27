const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const source = path.resolve(__dirname, '../ui');

test('sidebar resizing shares geometry, persists, cancels and preserves navigation in both layouts', async () => {
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
    const mode = page.locator('#sidebar-resize-mode');
    const handle = page.locator('.sidebar > .sidebar-resizer');
    const width = () => page.locator('.sidebar').evaluate(n => n.getBoundingClientRect().width);
    const select = async value => { await page.locator('#advanced-nav').click(); await page.locator('#advanced-experiments-tab').click(); await mode.click(); await page.locator(`#sidebar-resize-options [data-value="${value}"]`).click(); };
    assert.equal(await mode.evaluate(n => n.value), 'off'); assert.equal(await handle.isVisible(), false);
    await select('standard'); assert.equal(await width(), 256);
    assert.deepEqual(await page.locator('#sidebar-resize-options .kanban-option-label').allTextContents(), ['Off', 'Variant 1', 'Variant 2']);
    assert.deepEqual(await page.locator('#sidebar-resize-options .kanban-select-option-description').allTextContents(), ['Resizable Sidebar', 'Resizable Sidebar pro']);
    await mode.click();
    await page.locator('#sidebar-resize-options').evaluate(async n => { await Promise.allSettled(n.getAnimations().map(a => a.finished)); });
    await page.screenshot({ path: path.resolve(source, '../../.test-run/sidebar-picker-dark.png') });
    await page.keyboard.press('Escape');
    await handle.focus(); await page.keyboard.press('End'); assert.equal(await width(), 320);
    assert.equal(await page.locator('#save-button').isVisible(), true);
    await page.locator('#save-button').click(); await page.waitForFunction(() => window.saved?.sidebarResizeMode === 'standard');
    await page.keyboard.press('Tab');
    await handle.focus(); await page.keyboard.press('Home'); assert.equal(await width(), 160);
    assert.equal(await page.locator('#save-button').isVisible(), false, 'Geometry does not dirty settings');
    await page.reload(); await page.waitForFunction(() => window.appReady); assert.equal(await width(), 160);
    await page.locator('#advanced-nav').hover();
    assert.equal(await page.locator('#sidebar-navigation-tooltip').textContent(), 'Advanced Settings');
    assert.equal(await page.locator('#advanced-nav > span').evaluate(n => getComputedStyle(n).whiteSpace), 'nowrap');
    await select('pro'); await handle.focus(); await page.keyboard.press('Home'); assert.equal(await width(), 56);
    await page.locator('#settings-v21-enabled').check();
    await page.locator('#preferences-nav').click();
    assert.equal(await page.locator('#preferences-nav').getAttribute('aria-expanded'), 'true');
    const child = page.locator('.v21-section-link').first(); await child.click();
    assert.equal(await child.getAttribute('data-sidebar-tooltip'), 'Accent');
    assert.equal(await page.locator('#preferences-nav').getAttribute('aria-label'), 'Preferences');
    assert.equal(await child.locator('.documentation-item-icon').isVisible(), true);
    assert.equal(await child.locator('.documentation-item-label').isVisible(), false);
    await child.hover();
    assert.equal(await page.locator('#sidebar-navigation-tooltip').isVisible(), true);
    assert.equal(await child.evaluate(n => getComputedStyle(n).borderRadius), '50%');
    await page.screenshot({ path: path.resolve(source, '../../.test-run/sidebar-mini-tooltip-dark.png') });
    assert.ok(await page.locator('.window-title').evaluate(n => n.getBoundingClientRect().left + parseFloat(getComputedStyle(n).paddingLeft)) > 56);
    for (const name of ['log', 'help']) {
      await page.locator(`#${name}-button`).click();
      const popup = await page.locator(`#${name}-popup`).boundingBox();
      assert.ok(popup);
      assert.equal(await page.evaluate(({x,y}) => document.elementFromPoint(x,y)?.classList.contains('sidebar-resizer'), { x: 55, y: popup.y + popup.height / 2 }), false);
      await page.locator(`#${name}-button`).click();
    }
    const drag = async (target, cancel) => {
      const box = await handle.boundingBox(); await page.mouse.move(box.x + 4, box.y + 50); await page.mouse.down(); await page.mouse.move(target + box.x + 4 - await width(), box.y + 50);
      if (cancel) await page.keyboard.press('Escape');
      await page.mouse.up();
    };
    await drag(220, true); assert.equal(await width(), 56);
    await drag(220); assert.equal(await width(), 220);
    for (let i = 0; i < 6; i++) { await drag(i % 2 ? 160 : 320); }
    await drag(220);
    assert.equal(await page.locator('.preference-tabs').evaluate(n => getComputedStyle(n).overflowX), 'hidden');
    const tabState = await page.locator('.preference-tabs').evaluate(n => {
      const selected = n.querySelector('[aria-selected="true"]');
      return { left: parseFloat(n.style.getPropertyValue('--tab-left')), expected: selected.offsetLeft, width: parseFloat(n.style.getPropertyValue('--tab-width')), expectedWidth: selected.offsetWidth };
    });
    assert.equal(tabState.left, tabState.expected); assert.equal(tabState.width, tabState.expectedWidth);
    const box = await handle.boundingBox();
    await page.mouse.move(box.x + 4, box.y + 50); await page.mouse.down(); await page.mouse.move(300, box.y + 50);
    await handle.dispatchEvent('pointercancel'); await page.mouse.up(); assert.equal(await width(), 220);
    await drag(150); assert.equal(await width(), 56);
    await handle.focus(); await page.keyboard.press('ArrowRight'); assert.equal(await width(), 160);
    await page.keyboard.press('ArrowRight'); assert.equal(await width(), 168);
    await page.setViewportSize({ width: 600, height: 850 });
    await page.keyboard.press('End'); assert.equal(await width(), 240);
    await select('off'); assert.equal(await width(), 224);
    await page.evaluate(() => window.BlobUI.discardChanges()); assert.equal(await mode.evaluate(n => n.value), 'standard');
    await select('pro'); await page.evaluate(() => { window.failSave = true; }); await page.locator('#save-button').click();
    await page.waitForFunction(() => document.getElementById('status-message').textContent.includes('fixture save failed'));
    assert.equal(await mode.evaluate(n => n.value), 'pro');
    await page.evaluate(() => window.BlobUI.discardChanges()); assert.equal(await mode.evaluate(n => n.value), 'standard');
    // Show the real Docs layout without invoking a native app or external service.
    await page.evaluate(() => { document.querySelector('.documentation').hidden = false; });
    const docsHandle = page.locator('.documentation-layout > .sidebar-resizer');
    await docsHandle.focus(); await page.keyboard.press('Home');
    assert.equal(await page.locator('#documentation-navigation').evaluate(n => n.getBoundingClientRect().width), 160);
    assert.equal(await width(), 160);
    await page.evaluate(async () => {
      window.BlobUI.DocsData = { ...window.BlobUI.DocsData, videoAliases: {}, resolveImage() { return ""; }, load: async () => ({ categories: [{ id: 'features', name: 'Features' }], articles: [
        { id: 'parent', category: 'features', name: 'Parent guide', markdown: '# Parent guide', icon: 'features' },
        { id: 'child', category: 'features', parent: 'parent', name: 'Child guide', markdown: '# Child guide', icon: 'book' }
      ] }) };
      await window.BlobUI.DocsUI.load();
      window.BlobUI.SidebarResize.setMode('pro');
    });
    await docsHandle.focus(); await page.keyboard.press('Home');
    const parent = page.locator('[data-article-id="parent"]'); if (await parent.getAttribute("aria-expanded") !== "true") await parent.click();
    await page.locator('[data-article-id="child"]').click();
    assert.equal(await page.locator('#documentation-article h1').textContent(), 'Child guide');
    assert.equal(await parent.getAttribute('data-sidebar-tooltip'), 'Parent guide');
    assert.equal(await page.locator('[data-article-id="child"] .documentation-item-icon').isVisible(), true);
    await page.setViewportSize({ width: 1000, height: 850 });
    await page.screenshot({ path: path.resolve(source, '../../.test-run/sidebar-pro-docs-dark.png') });
    await page.evaluate(() => { document.querySelector('.documentation').hidden = true; });
    await page.screenshot({ path: path.resolve(source, '../../.test-run/sidebar-pro-settings-dark.png') });
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
        assert.equal(await page.locator('.sidebar-resizer').first().getAttribute('aria-valuenow'), '56');
        await page.evaluate(() => window.BlobUI.SidebarResize.setMode('standard'));
      }
      const handle = page.locator('.sidebar-resizer').first(); await handle.focus(); await page.keyboard.press('Home');
      assert.equal(await handle.getAttribute('aria-valuenow'), '160');
      await page.close();
    }
  } finally { await browser.close(); }
});
