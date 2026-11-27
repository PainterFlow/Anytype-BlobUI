const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

function createModalHarness() {
  const keydownHandlers = [];
  const timers = new Map();
  let nextTimer = 0;
  const document = {
    activeElement: null,
    documentElement: { dataset: {} },
    addEventListener(name, handler) {
      if (name === 'keydown') keydownHandlers.push(handler);
    },
  };
  const window = {
    matchMedia: () => ({ matches: false }),
    setTimeout(handler) {
      const id = ++nextTimer;
      timers.set(id, handler);
      return id;
    },
    clearTimeout(id) { timers.delete(id); },
  };

  class FakeElement {
    constructor(parent = null) {
      this.parentElement = parent;
      this.hidden = false;
      this.inert = false;
      this.disabled = false;
      this.attributes = {};
      this.handlers = {};
      this.classes = new Set();
      this.classList = {
        add: (name) => this.classes.add(name),
        remove: (name) => this.classes.delete(name),
        contains: (name) => this.classes.has(name),
      };
    }
    addEventListener(name, handler) { this.handlers[name] = handler; }
    getAttribute(name) { return this.attributes[name] ?? null; }
    setAttribute(name, value) { this.attributes[name] = value; }
    closest(selector) {
      if (selector !== '[hidden], [inert]') return null;
      for (let element = this; element; element = element.parentElement) {
        if (element.hidden || element.inert) return element;
      }
      return null;
    }
    getClientRects() { return this.closest('[hidden], [inert]') ? [] : [{}]; }
    focus() { document.activeElement = this; }
    querySelectorAll() { return this.focusable || []; }
  }

  function flushTimers() {
    const pending = [...timers.values()];
    timers.clear();
    pending.forEach((handler) => handler());
  }

  const context = { document, window, TypeError };
  vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../ui/modal.js'), 'utf8'), context);

  function createController({ target, initialFocus, fallbackFocus, inertTargets = [] } = {}) {
    const backdrop = new FakeElement();
    backdrop.hidden = true;
    const dialog = new FakeElement(backdrop);
    dialog.attributes.tabindex = '-1';
    const controls = [new FakeElement(dialog), new FakeElement(dialog), new FakeElement(dialog)];
    dialog.focusable = controls;
    const controller = context.window.BlobUI.createModalController({
      backdrop,
      dialog,
      initialFocus: initialFocus === undefined ? controls[0] : initialFocus(controls),
      fallbackFocus,
      inertTargets,
    });
    if (target) document.activeElement = target;
    return { backdrop, dialog, controls, controller };
  }

  function keydown(event) { keydownHandlers.forEach((handler) => handler(event)); }

  return { FakeElement, context, document, window, timers, flushTimers, createController, keydown };
}

const harness = createModalHarness();
const { FakeElement, document, window, createController, flushTimers, keydown } = harness;
const opener = new FakeElement();
const fallback = new FakeElement();
const shell = new FakeElement();
const titlebar = new FakeElement();
titlebar.inert = true;
const { backdrop, controls, controller } = createController({
  target: opener,
  fallbackFocus: fallback,
  inertTargets: [shell, titlebar],
});
controls[2].disabled = true;

controller.open();
assert.equal(document.activeElement, controls[0]);
assert.equal(shell.inert, true);
assert.equal(titlebar.inert, true);

for (const modifiers of [{ metaKey: true }, { altKey: true }, { ctrlKey: true }]) {
  for (const key of ['Tab', 'Escape']) {
    const systemShortcut = { key, ...modifiers, preventDefault() { this.defaultPrevented = true; } };
    keydown(systemShortcut);
    assert.equal(systemShortcut.defaultPrevented, undefined, 'Modified shortcuts must pass through the modal.');
    assert.equal(document.activeElement, controls[0], 'System shortcuts must not move modal focus.');
    assert.equal(backdrop.classList.contains('is-closing'), false, 'System shortcuts must not dismiss the modal.');
  }
}

const forwardTab = { key: 'Tab', shiftKey: false, preventDefault() { this.defaultPrevented = true; } };
keydown(forwardTab);
assert.equal(forwardTab.defaultPrevented, true);
assert.equal(document.activeElement, controls[1], 'Tab advances within the modal.');
const backwardTab = { key: 'Tab', shiftKey: true, preventDefault() { this.defaultPrevented = true; } };
keydown(backwardTab);
assert.equal(document.activeElement, controls[0], 'Shift+Tab moves back within the modal.');
const backwardWrap = { key: 'Tab', shiftKey: true, preventDefault() { this.defaultPrevented = true; } };
keydown(backwardWrap);
assert.equal(document.activeElement, controls[1], 'Shift+Tab wraps to the last enabled control.');
const forwardWrap = { key: 'Tab', shiftKey: false, preventDefault() { this.defaultPrevented = true; } };
keydown(forwardWrap);
assert.equal(document.activeElement, controls[0], 'Tab wraps to the first enabled control.');

controller.close();
assert.equal(backdrop.classList.contains('is-closing'), true);
controller.open();
assert.equal(backdrop.classList.contains('is-closing'), false, 'Reopening cancels the pending close animation.');
flushTimers();
assert.equal(backdrop.hidden, false, 'A canceled close timer cannot hide a reopened modal.');

const escape = { key: 'Escape', preventDefault() { this.defaultPrevented = true; } };
keydown(escape);
assert.equal(escape.defaultPrevented, true);
flushTimers();
assert.equal(backdrop.hidden, true, 'Escape dismisses the modal.');
assert.equal(shell.inert, false, 'The original inert state is restored.');
assert.equal(titlebar.inert, true, 'Pre-existing inert state is preserved.');
assert.equal(document.activeElement, opener, 'Focus returns to the visible invoker.');

const hiddenOpener = new FakeElement();
const fallbackHarness = createController({ target: hiddenOpener, fallbackFocus: fallback });
fallbackHarness.controller.open();
hiddenOpener.hidden = true;
fallbackHarness.controller.close();
flushTimers();
assert.equal(document.activeElement, fallback, 'A hidden invoker falls back to the provided heading.');

const reducedHarness = createController({ target: opener, inertTargets: [shell] });
document.documentElement.dataset.reducedMotion = 'true';
reducedHarness.controller.open();
reducedHarness.controller.close();
assert.equal(reducedHarness.backdrop.hidden, true, 'Reduced motion closes immediately.');
assert.equal(shell.inert, false, 'Immediate close restores inert state.');

const backdropHarness = createController({ target: opener });
document.documentElement.dataset.reducedMotion = 'false';
backdropHarness.controller.open();
backdropHarness.backdrop.handlers.pointerdown({ target: backdropHarness.backdrop });
assert.equal(backdropHarness.backdrop.classList.contains('is-closing'), true, 'Backdrop click dismisses the modal.');

console.log('modal lifecycle tests passed');
