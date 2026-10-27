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
  for (const name of ['visibility-field', 'modal', 'tabs', 'disclosure', 'pick-control', 'expandable-search', 'components', 'scrollbar']) {
    await page.addScriptTag({ path: path.join(ui, name + '.js') });
  }
  return page;
}

test('field updates preserve identity, focus and selection without emitting user changes', async () => {
  const page = await fixture();
  try {
    const result = await page.evaluate(() => {
      const C = BlobUI.Components;
      let inputs = 0, changes = 0;
      const field = C.TextField({ label: 'API key', inputAttributes: { id: 'key', type: 'text' }, value: 'abcdef', onInput: () => inputs++ });
      const toggle = C.ToggleSetting({ id: 'toggle', label: 'Enabled', onChange: () => changes++ });
      document.getElementById('host').append(field.element, toggle.element);
      field.input.focus(); field.input.setSelectionRange(2, 4);
      field.update({ value: 'abcdef', disabled: false });
      toggle.update({ checked: true });
      const preserved = document.activeElement === field.input && field.input.selectionStart === 2 && field.input.selectionEnd === 4;
      const silent = inputs === 0 && changes === 0;
      toggle.input.click();
      field.input.dispatchEvent(new Event('input', { bubbles: true }));
      const delivered = inputs === 1 && changes === 1;
      toggle.destroy(); toggle.destroy(); field.destroy();
      toggle.input.click(); field.input.dispatchEvent(new Event('input'));
      toggle.update({ label: 'Should not change' });
      return { preserved, silent, delivered, cleaned: inputs === 1 && changes === 1, label: toggle.element.querySelector('span').textContent,
        labelled: field.element.querySelector('label').htmlFor === field.input.id };
    });
    assert.deepEqual(result, { preserved: true, silent: true, delivered: true, cleaned: true, label: 'Enabled', labelled: true });
  } finally { await page.close(); }
});

test('destroying a visibility field cancels a pending secret reveal and permits clean reattachment', async () => {
  const page = await fixture();
  try {
    const result = await page.evaluate(async () => {
      const field = BlobUI.Components.TextField({ label: 'Secret', inputAttributes: { id: 'secret', type: 'password' },
        visibility: { buttonAttributes: { id: 'eye', 'aria-label': 'Show secret' } } });
      document.getElementById('host').append(field.element);
      let resolve, errors = 0;
      field.bindVisibility({ defaultVisible: false, showLabel: 'Show secret', hideLabel: 'Hide secret', shouldReveal: () => true,
        reveal: () => new Promise(done => { resolve = done; }), onRevealError: () => errors++ });
      field.button.click(); field.destroy(); resolve('decrypted secret');
      await Promise.resolve(); await Promise.resolve();
      const cancelled = field.input.value === '' && field.input.type === 'password';
      const next = VisibilityField.bind({ input: field.input, button: field.button, field: field.field, defaultVisible: false, showLabel: 'Show secret', hideLabel: 'Hide secret' });
      field.button.click();
      const visible = field.input.type === 'text'; next.destroy();
      return { cancelled, visible, errors };
    });
    assert.deepEqual(result, { cancelled: true, visible: true, errors: 0 });
  } finally { await page.close(); }
});

test('dialog destruction restores focus and background interaction and removes key handlers', async () => {
  const page = await fixture();
  try {
    const result = await page.evaluate(() => {
      const C = BlobUI.Components, host = document.getElementById('host');
      const trigger = C.Button({ label: 'Open' }); host.append(trigger.element); trigger.element.focus();
      const close = C.Button({ label: 'Close' });
      const shell = C.DialogShell({ dialogAttributes: { 'aria-label': 'Test dialog' }, content: [close.element] });
      document.body.append(shell.element);
      const modal = shell.attach({ initialFocus: close.element, inertTargets: [host] }); modal.open();
      const opened = host.inert && document.activeElement === close.element;
      shell.destroy(); shell.destroy(); modal.open();
      const event = new KeyboardEvent('keydown', { key: 'Escape', cancelable: true }); document.dispatchEvent(event);
      return { opened, hidden: shell.element.hidden, restored: !host.inert && document.activeElement === trigger.element, detached: !event.defaultPrevented };
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
