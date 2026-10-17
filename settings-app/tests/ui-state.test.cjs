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
      clientHeight: 500, scrollHeight: 500,
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
const timers = new Map();
let nextTimerId = 1;
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
    querySelector: selector => selector === '.page-content' ? element('page-content') : null,
    createElement: () => element('row-' + elements.size),
    querySelectorAll: () => ['accent-hue', 'accent-saturation', 'accent-brightness', 'native-accent', 'accent-value', 'font-family', 'code-font-family'].map(element),
  },
  CSS: { supports: () => true },
  window: {
    handlers: {},
    CSS: { supports: () => true },
    setInterval() {},
    setTimeout(callback, delay) {
      const id = nextTimerId++;
      timers.set(id, { callback, delay });
      return id;
    },
    clearTimeout(id) { timers.delete(id); },
    addEventListener(name, handler) { this.handlers[name] = handler; },
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

  const pageContent = element('page-content');
  assert.equal(pageContent.classList.contains('has-overflow'), false, 'A page without overflow starts without a scrollbar.');
  pageContent.handlers.scroll();
  assert.equal(pageContent.classList.contains('is-scrolling'), false, 'A page without overflow must not show a scrollbar.');
  pageContent.scrollHeight = 900;
  context.window.handlers.resize();
  assert.equal(pageContent.classList.contains('has-overflow'), true, 'The page must expose its scrollbar when content overflows.');
  pageContent.handlers.scroll();
  assert.equal(pageContent.classList.contains('is-scrolling'), true, 'The scrollbar must appear during scrolling.');
  const firstScrollbarTimerId = [...timers.keys()][0];
  pageContent.handlers.scroll();
  assert.equal(timers.has(firstScrollbarTimerId), false, 'A new scroll event must restart the idle delay.');
  assert.equal(timers.size, 1);
  const [scrollbarTimerId, scrollbarTimer] = [...timers.entries()][0];
  assert.equal(scrollbarTimer.delay, 900);
  timers.delete(scrollbarTimerId);
  scrollbarTimer.callback();
  assert.equal(pageContent.classList.contains('is-scrolling'), false, 'The scrollbar must hide after scrolling stops.');
  pageContent.handlers.scroll();
  assert.equal(pageContent.classList.contains('is-scrolling'), true);
  pageContent.scrollHeight = 500;
  context.window.handlers.resize();
  assert.equal(pageContent.classList.contains('has-overflow'), false, 'The page must hide its scrollbar when content no longer overflows.');
  assert.equal(pageContent.classList.contains('is-scrolling'), false, 'The scrollbar must disappear if overflow ends during a scroll.');
  assert.equal(timers.size, 0, 'An obsolete idle timer must be cleared when overflow ends.');

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
  const welcome = element('onboarding-welcome');
  const shell = element('settings-shell');
  context.document.activeElement = element('font-family');
  const beforeOnboarding = calls.length;
  const unsavedFont = element('font-family').value;
  const debugKey = { key: 'F8', preventDefault() {} };
  context.document.handlers.keydown({ ...debugKey, repeat: true });
  assert.equal(welcome.hidden, true, 'Key repeat must not open onboarding.');
  context.document.handlers.keydown({ ...debugKey, ctrlKey: true });
  assert.equal(welcome.hidden, true, 'Modified F8 must not open onboarding.');
  context.document.handlers.keydown(debugKey);
  assert.equal(welcome.hidden, false);
  assert.equal(shell.inert, true, 'Covered settings must not receive keyboard focus.');
  assert(welcome.focused);
  welcome.handlers.click();
  assert.equal(welcome.hidden, true);
  assert.equal(shell.inert, false);
  assert(element('font-family').focused, 'Continue must restore previous focus.');
  context.document.handlers.keydown(debugKey);
  assert.equal(welcome.hidden, false, 'F8 must reopen onboarding after continuing.');
  welcome.handlers.click();
  assert.equal(element('font-family').value, unsavedFont, 'Onboarding must preserve edits.');
  assert.equal(calls.length, beforeOnboarding, 'Onboarding must not save or rebuild settings.');
  console.log('UI save visibility, log severity, popup, copy, and window action tests passed.');
})().catch(error => { console.error(error); process.exitCode = 1; });
