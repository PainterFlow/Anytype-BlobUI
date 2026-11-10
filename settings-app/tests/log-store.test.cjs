const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');

const window = { BlobUI: {} };
vm.runInNewContext(fs.readFileSync(path.join(__dirname, '../ui/log-store.js'), 'utf8'), { window });

test('log store reports Anytype connection failures once per attempt as warnings', () => {
  const store = window.BlobUI.LogStore.create();
  const message = "Could not reach Anytype's Local API. Confirm that Anytype is open and Local API is enabled.";

  assert.equal(store.add(message, 'error', '10:00:00'), true);
  assert.equal(store.add(message, 'info', '10:00:01'), false);
  assert.equal(store.entries().length, 1);
  assert.equal(store.entries()[0].level, 'warning');
  assert.equal(store.severity(), 'warning');

  store.beginAttempt();
  assert.equal(store.add(message, 'info', '10:00:02'), true);
  assert.equal(store.add("The request to Anytype's Local API timed out. Confirm that Anytype is open and Local API is enabled.", 'info', '10:00:03'), false,
    'Different connection failure messages collapse into one warning per attempt.');
  assert.equal(store.entries().length, 2);

  store.beginAttempt();
  assert.equal(store.add(message, 'info', '10:00:04'), true, 'A later refresh can record the failure again.');
  assert.equal(window.BlobUI.LogStore.isConnectionFailure(message), true);
  assert.equal(window.BlobUI.LogStore.isConnectionFailure('Enter an Anytype API key.'), false);
});

test('log store upgrades duplicate messages and clear resets rows, severity, and deduplication', () => {
  const store = window.BlobUI.LogStore.create();
  assert.equal(store.add('Settings could not be saved.', 'info', '10:00:00'), true);
  assert.equal(store.add('Settings could not be saved.', 'error', '10:00:01'), true);
  assert.equal(store.entries().length, 1);
  assert.equal(store.entries()[0].level, 'error');
  assert.equal(store.severity(), 'error');

  store.clear();
  assert.equal(store.entries().length, 0);
  assert.equal(store.severity(), 'info');
  assert.equal(store.add('Settings could not be saved.', 'error', '10:00:02'), true);
});
