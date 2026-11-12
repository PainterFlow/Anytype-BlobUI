const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const window = { BlobUI: {} };
vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../ui/log-store.js'), 'utf8'), { window });

test('log store reports identical connection failures from separate throws separately', () => {
  const store = window.BlobUI.LogStore.create();
  const message = "Could not reach Anytype's Local API. Confirm that Anytype is open and Local API is enabled.";

  const firstError = new Error(message);
  const secondError = new Error(message);
  assert.equal(store.add(message, 'error', firstError, '10:00:00'), true);
  assert.equal(store.add(message, 'info', firstError, '10:00:01'), false);
  assert.equal(store.entries().length, 1);
  assert.equal(store.entries()[0].level, 'warning');
  assert.equal(store.severity(), 'warning');

  assert.equal(store.add(message, 'info', secondError, '10:00:02'), true,
    'A different throw is logged even when its message matches.');
  assert.equal(store.add("The request to Anytype's Local API timed out. Confirm that Anytype is open and Local API is enabled.", 'info', new Error('timeout'), '10:00:03'), true,
    'A different connection error message is also logged.');
  assert.equal(store.entries().length, 3);
  assert.equal(window.BlobUI.LogStore.isConnectionFailure(message), true);
  assert.equal(window.BlobUI.LogStore.isConnectionFailure('Enter an Anytype API key.'), false);
});

test('log store logs one row per thrown error and clear resets rows and severity', () => {
  const store = window.BlobUI.LogStore.create();
  const thrown = new Error('Settings could not be saved.');
  assert.equal(store.add('Settings could not be saved.', 'info', thrown, '10:00:00'), true);
  assert.equal(store.add('Settings could not be saved.', 'error', thrown, '10:00:01'), true);
  assert.equal(store.entries().length, 1);
  assert.equal(store.entries()[0].level, 'error');
  assert.equal(store.severity(), 'error');

  store.clear();
  assert.equal(store.entries().length, 0);
  assert.equal(store.severity(), 'info');
  assert.equal(store.add('Settings could not be saved.', 'error', thrown, '10:00:02'), true);
});
