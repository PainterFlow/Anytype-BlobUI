const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

// Exercise the actual event handlers without opening a browser or desktop window.
const elements = new Map();
function element(id) {
  if (!elements.has(id)) {
    const classes = new Set();
    elements.set(id, {
      value: '', textContent: '', hidden: true, children: [], attributes: {}, style: { setProperty() {} }, dataset: {},
      classList: {
        add: (...names) => names.forEach(name => classes.add(name)),
        remove: (...names) => names.forEach(name => classes.delete(name)),
        contains: name => classes.has(name),
        toggle(name, active) { active ? classes.add(name) : classes.delete(name); },
      },
      handlers: {},
      addEventListener(name, handler) { this.handlers[name] = handler; },
      setAttribute(name, value) { this.attributes[name] = value; },
      appendChild(child) { this.children.push(child); },
      contains(target) { return target === this || this.children.includes(target); },
      focus() { this.focused = true; },
      querySelector: selector => element(id + selector),
    });
  }
  return elements.get(id);
}
const calls = [];
let saveError = null;
let copiedLogs = '';
let copyError = false;
const context = {
  navigator: { clipboard: { async writeText(text) { if (copyError) throw new Error('Clipboard unavailable.'); copiedLogs = text; } } },
  document: {
    handlers: {},
    addEventListener(name, handler) { this.handlers[name] = handler; },
    documentElement: element('root'),
    getElementById: element,
    createElement: () => element('row-' + elements.size),
    querySelectorAll: () => ['accent-hue', 'accent-saturation', 'accent-brightness', 'native-accent', 'accent-value', 'font-family', 'code-font-family'].map(element),
  },
  CSS: { supports: () => true },
  window: {
    CSS: { supports: () => true },
    setInterval() {},
    matchMedia: () => ({ matches: false, addEventListener() {} }),
    __TAURI__: { core: { async invoke(name) {
      calls.push(name);
      if (name === 'get_app_state') return {
        ready: true, theme: 'dark',
        settings: { fontFamily: 'Libron', codeFontFamily: 'Consolas', accentColor: 'Blob Pink' },
      };
      if (name === 'save_settings') {
        if (saveError) throw new Error(saveError);
        return { calendarStatus: 'cached' };
      }
    } } },
  },
};
const flush = () => new Promise(resolve => setImmediate(resolve));
const input = async (id, value) => {
  element(id).value = value;
  await element(id).handlers.input();
};
const status = () => element('status-message').textContent;

(async () => {
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../ui/app.js'), 'utf8'), context);
  assert.equal(element('log-button').hidden, true, 'Logs stay hidden before any messages arrive.');
  await flush();
  assert.equal(status(), 'Settings loaded from this device.');
  assert.equal(element('save-button').hidden, true);
  assert.equal(element('log-button').hidden, false);
  assert.equal(element('log-button').dataset.severity, 'info');
  assert.equal(element('log-entries').children.length, 1);

  await input('font-family', 'Arial');
  assert.match(status(), /Unsaved changes/);
  assert.equal(element('save-button').disabled, false);
  assert.equal(element('save-button').hidden, false);
  await input('font-family', 'Libron');
  assert.equal(status(), 'No unsaved changes.');
  assert.equal(element('save-button').hidden, true);
  await input('accent-value', '#123456');
  assert.equal(element('save-button').hidden, false);
  await input('accent-value', '#DE50DE');
  assert.equal(element('save-button').hidden, true);
  assert.equal(element('log-entries').children.length, 1, 'Editing should not fill the log.');

  await input('font-family', 'Arial');
  const pendingSave = element('save-button').handlers.click();
  assert.equal(element('save-button').hidden, false, 'Pending changes remain visible while saving.');
  assert.equal(element('save-button').disabled, true, 'Repeated saves must be blocked.');
  await pendingSave;
  assert.match(status(), /^Saved\./);
  assert.equal(element('save-button').hidden, true);
  assert.equal(element('log-button').dataset.severity, 'warning');
  await input('font-family', 'Libron');
  saveError = '<script>Rebuild failed.</script>';
  await element('save-button').handlers.click();
  assert.equal(status(), saveError);
  assert.equal(element('save-button').hidden, false);
  assert.equal(element('log-button').dataset.severity, 'error');
  assert(element('log-entries').children.at(-1).textContent.endsWith(saveError), 'Log messages must remain plain text.');
  saveError = null;
  await element('save-button').handlers.click();
  assert.equal(element('log-button').dataset.severity, 'error', 'Retained errors take precedence over new warnings.');
  assert.equal(element('save-button').hidden, true);

  element('log-button').handlers.click();
  assert.equal(element('log-popup').hidden, false);
  assert.equal(element('log-button').attributes['aria-expanded'], 'true');
  assert(element('copy-logs').focused);
  await element('copy-logs').handlers.click();
  assert(copiedLogs.includes('Rebuild failed.'));
  assert.equal(copiedLogs.split('\n').length, element('log-entries').children.length);
  context.document.handlers.pointerdown({ target: element('log-control') });
  assert.equal(element('log-popup').hidden, false);
  context.document.handlers.pointerdown({ target: element('font-family') });
  assert.equal(element('log-popup').hidden, true);
  element('log-button').handlers.click();
  context.document.handlers.keydown({ key: 'Escape', preventDefault() {} });
  assert.equal(element('log-popup').hidden, true);
  assert(element('log-button').focused);
  element('log-button').handlers.click();
  context.document.handlers.focusin({ target: element('font-family') });
  assert.equal(element('log-popup').hidden, true);
  copyError = true;
  await element('copy-logs').handlers.click();
  assert.match(element('copy-status').textContent, /Could not copy/);
  assert.match(element('log-entries').children.at(-1).textContent, /Clipboard unavailable/);

  for (const [id, command] of [
    ['window-minimize', 'minimize_window'], ['window-close', 'hide_window'],
    ['open-repository', 'open_project_repository'],
    ['open-author', 'open_author_profile'],
  ]) {
    await element(id).handlers.click();
    assert.equal(calls.at(-1), command);
  }
  let prevented = false;
  const quitKey = { key: 'q', ctrlKey: true, preventDefault() { prevented = true; } };
  context.document.handlers.keydown(quitKey);
  assert.equal(calls.at(-1), 'quit_app');
  assert(prevented);
  const beforeIgnoredKeys = calls.length;
  context.document.handlers.keydown({ ...quitKey, ctrlKey: false });
  context.document.handlers.keydown({ ...quitKey, repeat: true });
  context.document.handlers.keydown({ ...quitKey, altKey: true });
  assert.equal(calls.length, beforeIgnoredKeys);
  element('title-drag-region').handlers.pointerdown({ button: 0, target: { closest: () => null } });
  assert.equal(calls.at(-1), 'start_window_drag');
  const count = calls.length;
  element('title-drag-region').handlers.pointerdown({ button: 0, target: { closest: () => ({}) } });
  assert.equal(calls.length, count, 'Window control clicks must not start a drag.');
  console.log('UI save visibility, log severity, popup, copy, and window action tests passed.');
})().catch(error => { console.error(error); process.exitCode = 1; });
