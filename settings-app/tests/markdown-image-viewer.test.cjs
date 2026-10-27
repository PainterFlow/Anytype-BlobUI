const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const ui = path.resolve(__dirname, '../ui');

test('images expand at source resolution, restore focus, and preserve an underlying modal', async () => {
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 960, height: 740 } });
    const errors = []; page.on('pageerror', error => errors.push(error.message));
    await page.setContent('<!doctype html><html data-theme="dark" data-reduced-motion="true"><body><main id="docs" class="markdown-content"></main><div id="underlying" class="confirm-backdrop" hidden><section id="under-dialog" class="confirm-dialog" role="dialog" aria-modal="true" aria-label="Release notes" tabindex="-1"><button id="under-close">Close release notes</button><main id="notes" class="markdown-content"></main></section></div></body></html>');
    for (const name of ['styles.css', 'markdown-image-viewer.css']) await page.addStyleTag({ path: path.join(ui, name) });
    for (const name of ['modal.js', 'vendor/marked.umd.js', 'markdown.js']) await page.addScriptTag({ path: path.join(ui, name) });
    // Register the older dialog first to exercise the inert-modal keydown guard.
    await page.evaluate(() => {
      window.underModal = BlobUI.createModalController({ backdrop: document.getElementById('underlying'), dialog: document.getElementById('under-dialog'),
        initialFocus: document.getElementById('under-close'), inertTargets: [document.getElementById('docs')] });
    });
    await page.addScriptTag({ path: path.join(ui, 'markdown-image-viewer.js') });
    const src = 'data:image/png;base64,' + fs.readFileSync(path.join(ui, 'cdn/calendar-event-colors.png')).toString('base64');
    await page.evaluate(source => {
      window.images = { picture: { src: source } };
      BlobUI.Markdown.mount(document.getElementById('docs'), '![Calendar](@picture)\n\n[![Linked](@picture)](https://example.com)', { images });
      BlobUI.Markdown.mount(document.getElementById('notes'), '![Release image](@picture)', { images });
    }, src);
    const source = page.locator('#docs img').first();
    assert.equal(await source.getAttribute('role'), 'button');
    assert.equal(await page.locator('#docs a img').getAttribute('role'), null);
    await source.click();
    const viewer = page.locator('.markdown-image-viewer-backdrop');
    const preview = viewer.locator('.markdown-image-viewer-image');
    await page.waitForFunction(() => document.querySelector('.markdown-image-viewer-image').naturalWidth === 2028);
    assert.equal(await page.locator('#docs').evaluate(node => node.inert), true);
    const fit = await preview.evaluate(image => {
      const stage = image.parentElement.getBoundingClientRect(), bounds = image.getBoundingClientRect();
      return bounds.width <= stage.width && bounds.height <= stage.height;
    });
    assert.equal(fit, true);
    await viewer.getByRole('button', { name: 'Show actual size' }).click();
    assert.equal(await preview.evaluate(image => image.getBoundingClientRect().width), 2028);
    const panned = await viewer.locator('.markdown-image-viewer-stage').evaluate(stage => {
      stage.scrollLeft = 150; return stage.scrollWidth > stage.clientWidth && stage.scrollLeft === 150;
    });
    assert.equal(panned, true);
    await page.screenshot({ path: path.resolve(__dirname, '../../.test-run/image-viewer-native-dark.png') });
    await viewer.getByRole('button', { name: 'Fit to window' }).click();
    await page.keyboard.press('Escape');
    await viewer.waitFor({ state: 'hidden' });
    assert.equal(await source.evaluate(node => document.activeElement === node), true);
    assert.equal(await page.locator('#docs').evaluate(node => node.inert), false);
    await source.focus(); await page.keyboard.press('Space');
    await viewer.waitFor({ state: 'visible' });
    await viewer.getByRole('button', { name: 'Close image preview' }).click();
    await viewer.waitFor({ state: 'hidden' });
    await page.evaluate(() => underModal.open());
    await page.locator('#notes img').click();
    await viewer.waitFor({ state: 'visible' });
    assert.equal(await page.locator('#underlying').evaluate(node => node.inert), true);
    await page.keyboard.press('Tab');
    assert.equal(await viewer.evaluate(node => node.contains(document.activeElement)), true);
    await page.keyboard.press('Escape');
    await viewer.waitFor({ state: 'hidden' });
    assert.equal(await page.locator('#underlying').evaluate(node => !node.hidden && !node.inert), true);
    assert.equal(await page.locator('#notes img').evaluate(node => document.activeElement === node), true);
    await page.locator('#notes img').click();
    await viewer.waitFor({ state: 'visible' });
    await page.evaluate(() => BlobUI.Markdown.mount(document.getElementById('notes'), '# Replacement'));
    await viewer.waitFor({ state: 'hidden' });
    assert.equal(await preview.getAttribute('src'), null);
    assert.equal(await page.locator('#underlying').evaluate(node => node.inert), false);
    await page.keyboard.press('Escape');
    assert.equal(await page.locator('#underlying').evaluate(node => node.hidden), true);
    assert.deepEqual(errors, []);
  } finally { await browser.close(); }
});
