const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const test = require('node:test');

const source = fs.readFileSync(path.join(__dirname, '../ui/reload-controller.js'), 'utf8');

function harness(options = {}) {
  const documentHandlers = new Map();
  const buttonHandlers = new Map();
  const document = {
    addEventListener(type, handler) { documentHandlers.set(type, handler); },
    removeEventListener(type, handler) { if (documentHandlers.get(type) === handler) documentHandlers.delete(type); },
    fire(type, event) { documentHandlers.get(type)?.(event); },
  };
  const button = {
    attributes: {},
    addEventListener(type, handler) { buttonHandlers.set(type, handler); },
    removeEventListener(type, handler) { if (buttonHandlers.get(type) === handler) buttonHandlers.delete(type); },
    setAttribute(name, value) { this.attributes[name] = value; },
    click() { buttonHandlers.get('click')?.(); },
  };
  const window = { document };
  vm.runInNewContext(source, { window });
  return { controller: window.BlobUI.ReloadController.create({ button, ...options }), document, button };
}

test('reload runs are deduplicated while busy and always release busy state', async () => {
  const busy = [];
  const { controller, button } = harness({ onBusyChange: value => busy.push(value) });
  let resolveHandler;
  let calls = 0;
  controller.register(() => {
    calls += 1;
    return new Promise(resolve => { resolveHandler = resolve; });
  });

  const first = controller.run();
  const second = controller.run();
  assert.equal(first, second);
  await Promise.resolve();
  assert.equal(calls, 1);
  assert.equal(button.attributes['aria-busy'], 'true');
  resolveHandler();
  await first;
  assert.equal(button.attributes['aria-busy'], 'false');
  assert.deepEqual(busy, [true, false]);
  controller.destroy();
});

test('reload reports synchronous and asynchronous handler errors without leaving busy state set', async () => {
  for (const [handler, expected] of [
    [() => { throw new Error('synchronous failure'); }, 'synchronous failure'],
    [() => Promise.reject(new Error('asynchronous failure')), 'asynchronous failure'],
  ]) {
    const errors = [];
    const { controller, button } = harness({ onError: error => errors.push(error.message) });
    controller.register(handler);
    await controller.run();
    assert.equal(button.attributes['aria-busy'], 'false');
    assert.deepEqual(errors, [expected]);
    controller.destroy();
  }
});

test('Ctrl+R reloads the app, while other modifiers pass through', async () => {
  const { controller, document, button } = harness();
  let calls = 0;
  controller.register(() => { calls += 1; });
  const event = overrides => ({ key: 'r', ctrlKey: true, metaKey: false, altKey: false, shiftKey: false, prevented: false, preventDefault() { this.prevented = true; }, ...overrides });

  for (const modifier of [{ altKey: true }, { shiftKey: true }, { metaKey: true }, { ctrlKey: false }]) {
    const ignored = event(modifier);
    document.fire('keydown', ignored);
    assert.equal(ignored.prevented, false);
  }
  const reload = event();
  document.fire('keydown', reload);
  assert.equal(reload.prevented, true);
  await new Promise(setImmediate);
  assert.equal(calls, 1);

  button.click();
  await new Promise(setImmediate);
  assert.equal(calls, 2);
  controller.destroy();
  const afterDestroy = event();
  document.fire('keydown', afterDestroy);
  assert.equal(afterDestroy.prevented, false);
});
