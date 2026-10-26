const assert = require('node:assert/strict');
const path = require('node:path');
const test = require('node:test');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');

test('Docs and release-note links open externally without replacing the reader', async () => {
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true });
  try {
    const page = await browser.newPage();
    await page.setContent('<!doctype html><html data-theme="dark"><body style="background:#171717;color:#eee"><main id="docs"><a id="guide" href="https://example.com/guide"><span>Guide</span></a></main><section id="notes"><a id="release" href="https://example.com/release" target="_blank">Release</a></section><a id="hash" href="#heading">Heading</a><h2 id="heading">Section</h2></body></html>');
    await page.evaluate(() => {
      window.calls = []; window.errors = [];
      window.__TAURI__ = { core: { invoke: async (name, args) => {
        window.calls.push({ name, url: args.url });
        if (window.failOpen) throw Error('Fixture browser failure');
      } } };
    });
    await page.addScriptTag({ path: path.resolve(__dirname, '../ui/external-links.js') });
    await page.evaluate(() => { window.links = BlobUI.ExternalLinks.attach(document, { onError: error => window.errors.push(error.message) }); });
    const original = page.url();
    await page.locator('#guide span').click();
    await page.locator('#release').click({ modifiers: ['Control'] });
    await page.locator('#guide').click({ button: 'middle' });
    await page.locator('#release').focus(); await page.keyboard.press('Enter');
    await page.waitForFunction(() => calls.length === 4);
    assert.deepEqual(await page.evaluate(() => calls), [
      { name: 'open_external_link', url: 'https://example.com/guide' },
      { name: 'open_external_link', url: 'https://example.com/release' },
      { name: 'open_external_link', url: 'https://example.com/guide' },
      { name: 'open_external_link', url: 'https://example.com/release' },
    ]);
    assert.equal(page.url(), original);
    assert.equal(page.context().pages().length, 1);
    await page.locator('#hash').click();
    assert.equal(await page.evaluate(() => location.hash), '#heading');
    assert.equal(await page.evaluate(() => calls.length), 4);
    await page.evaluate(() => { window.failOpen = true; });
    await page.locator('#guide').click();
    await page.waitForFunction(() => errors.length === 1);
    assert.equal(await page.evaluate(() => errors[0]), 'Fixture browser failure');
    assert.equal(await page.evaluate(() => location.hash), '#heading');
    const afterCleanup = await page.evaluate(async () => {
      links.destroy(); links.destroy();
      // A guard keeps this isolated teardown check from navigating the test browser.
      document.addEventListener('click', event => event.preventDefault(), { once: true });
      document.getElementById('guide').click();
      await Promise.resolve(); await Promise.resolve();
      return calls.length;
    });
    assert.equal(afterCleanup, 5);
  } finally { await browser.close(); }
});
