const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

function makeElement() {
  return {
    value: '', type: 'password', disabled: false, dataset: {}, attributes: {}, handlers: {},
    selectionStart: 0, selectionEnd: 0,
    addEventListener(name, handler) { this.handlers[name] = handler; },
    setAttribute(name, value) { this.attributes[name] = value; },
    setSelectionRange(start, end) { this.selectionStart = start; this.selectionEnd = end; },
  };
}

const input = makeElement();
const button = makeElement();
const field = makeElement();
let activeElement = input;
const document = { get activeElement() { return activeElement; } };
const context = { document, window: {} };
vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../ui/visibility-field.js'), 'utf8'), context);
(async () => {
const component = context.window.VisibilityField.bind({
  input, button, field,
  showLabel: 'Show value', hideLabel: 'Hide value', defaultVisible: false,
});

assert.equal(input.type, 'password');
assert.equal(button.attributes['aria-pressed'], 'false');
assert.equal(button.attributes['aria-label'], 'Show value');
assert.equal(button.attributes.title, 'Show value');
assert.equal(field.dataset.filled, 'false');

input.value = 'secret';
input.handlers.input();
assert.equal(field.dataset.filled, 'true');
input.selectionStart = 1;
input.selectionEnd = 4;
const preventDefault = () => {};
button.handlers.mousedown({ preventDefault });
await button.handlers.click({ preventDefault });
assert.equal(input.type, 'text');
assert.equal(button.attributes['aria-pressed'], 'true');
assert.equal(button.attributes['aria-label'], 'Hide value');
assert.equal(activeElement, input);
assert.deepEqual([input.selectionStart, input.selectionEnd], [1, 4]);
await button.handlers.click({ preventDefault });
assert.equal(input.type, 'password');
assert.equal(input.value, 'secret');
assert.equal(typeof component.reset, 'function');
component.reset();
assert.equal(input.type, 'password');

let resolveReveal;
let busy = [];
let revealed = false;
const revealInput = makeElement();
const revealButton = makeElement();
const revealField = makeElement();
const revealComponent = context.window.VisibilityField.bind({
  input: revealInput,
  button: revealButton,
  field: revealField,
  showLabel: 'Show saved value',
  hideLabel: 'Hide saved value',
  defaultVisible: false,
  shouldReveal: () => true,
  reveal: () => new Promise(resolve => { resolveReveal = resolve; }),
  onBusyChange: value => busy.push(value),
  onHide: () => { revealed = false; revealInput.value = '••••••••'; },
});
revealInput.value = '••••••••';
revealComponent.sync();
const pendingClick = revealButton.handlers.click({ preventDefault });
assert.equal(revealButton.disabled, false, 'Busy state is communicated through the injected callback.');
assert.deepEqual(busy, [true]);
revealInput.value = 'new user value';
revealInput.handlers.input();
assert.deepEqual(busy, [true, false], 'Editing invalidates the pending reveal and clears busy state.');
resolveReveal('stale saved value');
await pendingClick;
assert.equal(revealInput.value, 'new user value', 'An async reveal must not overwrite a newer input value.');
assert.equal(revealInput.type, 'password', 'A stale reveal must not change visibility.');
assert.equal(revealed, false);

const resetClick = revealButton.handlers.click({ preventDefault });
revealComponent.reset();
resolveReveal('stale after reset');
await resetClick;
assert.equal(revealInput.value, 'new user value', 'Reset invalidates a pending reveal.');
assert.equal(revealInput.type, 'password');

console.log('visibility-field tests passed');
})().catch(error => {
  console.error(error);
  process.exitCode = 1;
});
