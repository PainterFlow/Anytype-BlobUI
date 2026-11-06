const assert = require('node:assert/strict');
const path = require('node:path');
const { test, before, after } = require('node:test');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const ui = path.resolve(__dirname, '../ui');
let browser;
before(async () => { browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true }); });
after(async () => { await browser?.close(); });
async function fixture() {
  const page = await browser.newPage();
  await page.setContent('<!doctype html><html><body><main id="host"></main></body></html>');
  for (const name of ['visibility-field', 'modal', 'scrollbar']) {
    await page.addScriptTag({ path: path.join(ui, name + '.js') });
  }
  return page;
}

test('destroying a visibility field cancels a pending secret reveal and permits clean reattachment', async () => {
  const page = await fixture();
  try {
    const result = await page.evaluate(async () => {
      const host = document.getElementById('host');
      host.innerHTML = '<div id="field"><input id="secret" type="password"><button id="eye" type="button">Show secret</button></div>';
      const input = document.getElementById('secret'), button = document.getElementById('eye'), field = document.getElementById('field');
      const props = { input, button, field, defaultVisible: false, showLabel: 'Show secret', hideLabel: 'Hide secret' };
      let resolve, errors = 0;
      const controller = VisibilityField.bind({ ...props, shouldReveal: () => true,
        reveal: () => new Promise(done => { resolve = done; }), onRevealError: () => errors++ });
      button.click(); controller.destroy(); resolve('decrypted secret');
      await Promise.resolve(); await Promise.resolve();
      const cancelled = input.value === '' && input.type === 'password';
      const next = VisibilityField.bind(props);
      button.click();
      const visible = input.type === 'text'; next.destroy();
      return { cancelled, visible, errors };
    });
    assert.deepEqual(result, { cancelled: true, visible: true, errors: 0 });
  } finally { await page.close(); }
});

test('dialog destruction restores focus and background interaction and removes key handlers', async () => {
  const page = await fixture();
  try {
    const result = await page.evaluate(() => {
      const host = document.getElementById('host');
      host.innerHTML = '<button type="button">Open</button>';
      const trigger = host.firstElementChild;
      trigger.focus();
      const backdrop = document.createElement('div');
      backdrop.hidden = true;
      backdrop.innerHTML = '<section role="dialog" aria-label="Test dialog" tabindex="-1"><button type="button">Close</button></section>';
      document.body.append(backdrop);
      const dialog = backdrop.firstElementChild, close = dialog.firstElementChild;
      const modal = BlobUI.createModalController({ backdrop, dialog, initialFocus: close, inertTargets: [host] });
      modal.open();
      const opened = host.inert && document.activeElement === close;
      modal.destroy(); modal.destroy(); modal.open();
      const event = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }); document.dispatchEvent(event);
      return { opened, hidden: backdrop.hidden, restored: !host.inert && document.activeElement === trigger, detached: !event.defaultPrevented };
    });
    assert.deepEqual(result, { opened: true, hidden: true, restored: true, detached: true });
  } finally { await page.close(); }
});

test('scrollbars observe replacement content and disconnect on destruction', async () => {
  const page = await fixture();
  try {
    await page.evaluate(() => {
      const host = document.getElementById('host'); host.style.cssText = 'height:40px;overflow:auto';
      host.innerHTML = '<div class="content" style="height:10px"></div>';
      window.scrollbar = BlobUI.Scrollbar.attach(host, '.content');
      host.replaceChildren(Object.assign(document.createElement('div'), { className: 'content' }));
      host.firstElementChild.style.height = '100px';
    });
    await page.waitForFunction(() => document.getElementById('host').classList.contains('has-overflow'));
    const result = await page.evaluate(() => {
      const host = document.getElementById('host');
      host.dispatchEvent(new Event('scroll'));
      const scrolling = host.classList.contains('is-scrolling');
      scrollbar.destroy(); scrollbar.destroy();
      host.dispatchEvent(new Event('scroll'));
      return { scrolling, detached: !host.classList.contains('is-scrolling') };
    });
    assert.deepEqual(result, { scrolling: true, detached: true });
  } finally { await page.close(); }
});
