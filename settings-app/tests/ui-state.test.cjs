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
      value: '', textContent: '', style: { setProperty() {} }, dataset: {},
      classList: {
        add: (...names) => names.forEach(name => classes.add(name)),
        remove: (...names) => names.forEach(name => classes.delete(name)),
        contains: name => classes.has(name),
        toggle(name, active) { active ? classes.add(name) : classes.delete(name); },
      },
      handlers: {},
      addEventListener(name, handler) { this.handlers[name] = handler; },
      setAttribute() {},
      querySelector: selector => element(id + selector),
    });
  }
  return elements.get(id);
}
const timers = new Map();
const calls = [];
let nextTimer = 0;
let saveError = null;
const context = {
  document: {
    handlers: {},
    addEventListener(name, handler) { this.handlers[name] = handler; },
    documentElement: element('root'),
    getElementById: element,
    querySelectorAll: () => ['accent-hue', 'accent-saturation', 'accent-brightness', 'native-accent', 'accent-value', 'font-family', 'code-font-family'].map(element),
  },
  CSS: { supports: () => true },
  window: {
    CSS: { supports: () => true },
    setTimeout(callback, delay) { const id = ++nextTimer; timers.set(id, { callback, delay }); return id; },
    clearTimeout: id => timers.delete(id),
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
  await flush();
  assert.equal(status(), 'Settings loaded from this device.');
  assert.equal(timers.size, 1);
  const timer = [...timers.values()][0];
  assert.equal(timer.delay, 5000);
  timer.callback();
  assert(element('save-status').classList.contains('faded'));

  await input('font-family', 'Arial');
  assert.match(status(), /Unsaved changes/);
  assert.equal(element('save-button').disabled, false);
  assert(!element('save-status').classList.contains('faded'));
  await input('font-family', 'Libron');
  assert.equal(status(), 'No unsaved changes.');
  assert.equal(element('save-button').disabled, true);
  await input('accent-value', '#123456');
  assert.equal(element('save-button').disabled, false);
  await input('accent-value', '#DE50DE');
  assert.equal(element('save-button').disabled, true);
  assert.equal(status(), 'No unsaved changes.');

  await input('font-family', 'Arial');
  saveError = 'Rebuild failed.';
  await element('save-button').handlers.click();
  assert.equal(status(), saveError);
  assert.equal(timers.size, 0, 'Errors must cancel pending fades and remain visible.');
  assert(element('save-status').classList.contains('error'));
  assert(!element('save-status').classList.contains('faded'));
  saveError = null;
  await element('save-button').handlers.click();
  assert.match(status(), /^Saved\./);
  assert.equal(timers.size, 1);

  for (const [id, command] of [
    ['window-minimize', 'minimize_window'], ['window-close', 'hide_window'],
    ['open-repository', 'open_project_repository'],
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
  console.log('UI status, revert, save error, and window action tests passed.');
})().catch(error => { console.error(error); process.exitCode = 1; });
