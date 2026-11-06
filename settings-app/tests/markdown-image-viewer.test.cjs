const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { loadSettings } = require('./frontend-fixture.cjs');

test('images expand at source resolution, restore focus, and preserve the release notes dialog', async () => {
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 960, height: 740 }, colorScheme: 'dark' });
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    const source = 'data:image/png;base64,' + fs.readFileSync(path.resolve(__dirname, '../ui/cdn/calendar-event-colors.png')).toString('base64');
    await page.addInitScript(src => {
      window.__TAURI__ = { core: { invoke: async name => {
        if (name === 'get_app_state') return { ready: true, onboardingComplete: true, theme: 'dark', settings: { reducedMotion: true } };
        if (name === 'get_latest_release_notes') return {
          version: '1.2.0', markdown: '![Calendar](@picture)\n\n[![Linked](@picture)](https://example.com)',
          imageManifest: 'images:\n  - id: picture\n    src: assets/changelogs/calendar.png\n',
        };
        if (name === 'get_release_note_history_image') return src;
        return null;
      } } };
    }, source);
    await loadSettings(page);
    await page.waitForFunction(() => window.appReady);
    await page.locator('#help-button').click();
    await page.locator('#help-whats-new').click();
    const notes = page.locator('#release-notes-backdrop');
    const image = notes.getByRole('button', { name: 'Expand image: Calendar' });
    const viewer = page.locator('.markdown-image-viewer-backdrop');
    const preview = viewer.locator('.markdown-image-viewer-image');
    assert.equal(await notes.locator('a img').getAttribute('role'), null);
    await image.focus();
    await page.keyboard.press('Space');
    await viewer.waitFor({ state: 'visible' });
    await page.waitForFunction(() => document.querySelector('.markdown-image-viewer-image')?.naturalWidth === 2028);
    assert.equal(await notes.evaluate(node => Boolean(node.closest('[inert]'))), true);
    assert.equal(await preview.evaluate(node => {
      const stage = node.parentElement.getBoundingClientRect(), bounds = node.getBoundingClientRect();
      return bounds.width <= stage.width && bounds.height <= stage.height;
    }), true);
    await viewer.getByRole('button', { name: 'Show image at actual size' }).click();
    assert.equal(await preview.evaluate(node => node.getBoundingClientRect().width), 2028);
    const stage = viewer.locator('.markdown-image-viewer-stage');
    assert.equal(await stage.evaluate(node => {
      node.scrollLeft = 150;
      return node.scrollWidth > node.clientWidth && node.scrollLeft > 0;
    }), true);
    await page.screenshot({ path: path.resolve(__dirname, '../../.test-run/image-viewer-native-dark.png') });
    await viewer.getByRole('button', { name: 'Fit image to window' }).click();
    await page.keyboard.press('Tab');
    assert.equal(await viewer.evaluate(node => node.contains(document.activeElement)), true);
    await page.keyboard.press('Escape');
    await viewer.waitFor({ state: 'hidden' });
    assert.equal(await preview.count(), 0);
    assert.equal(await notes.evaluate(node => !node.hidden && !node.closest('[inert]')), true);
    assert.equal(await image.evaluate(node => document.activeElement === node), true);
    await page.keyboard.press('Escape');
    await notes.waitFor({ state: 'hidden' });
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
