const test = require('node:test');
const assert = require('node:assert/strict');
const { createNativeService } = require('../ui/native-service.js');

function serviceFor(tauri, onMetadataError = () => {}) {
  return createNativeService({ tauri, onMetadataError });
}

test('native service deduplicates metadata reads and tracks their promises', async () => {
  let resolveRead;
  let calls = 0;
  const service = serviceFor({ core: { invoke() { calls++; return new Promise(resolve => { resolveRead = resolve; }); } } });
  const first = service.invoke('discover_kanban_views', { spaceId: 'full-space-id' });
  const duplicate = service.invoke('discover_kanban_views', { spaceId: 'full-space-id' });
  assert.equal(first, duplicate);
  await Promise.resolve();
  assert.equal(calls, 1);
  assert.equal(service.metadataPending.has(first), true);
  resolveRead({ views: [] });
  assert.deepEqual(await first, { views: [] });
  await Promise.resolve();
  assert.equal(service.metadataPending.size, 0);
});

test('connection changes invalidate in-flight metadata and allow a new request', async () => {
  const resolves = [];
  const service = serviceFor({ core: { invoke(name) {
    if (name === 'discover_codex_spaces') return new Promise(resolve => resolves.push(resolve));
    return Promise.resolve(null);
  } } });
  const stale = service.invoke('discover_codex_spaces');
  await Promise.resolve();
  await service.invoke('configure_connection', { connection: { enabled: true } });
  const current = service.invoke('discover_codex_spaces');
  await Promise.resolve();
  resolves[0]([]);
  await assert.rejects(stale, /saved Anytype connection changed/);
  resolves[1](['space']);
  assert.deepEqual(await current, ['space']);
});

test('native events are injectable and metadata errors reach the caller', async () => {
  const errors = [];
  let listener;
  let calls = 0;
  const service = serviceFor({
    core: { async invoke() { return { errors: [{ error: 'catalog warning' }] }; } },
    event: { async listen(name, handler) { calls++; assert.equal(name, 'blobui-saved-state-changed'); listener = handler; return () => { listener = null; }; } },
  }, message => errors.push(message));
  await service.invoke('discover_anyblob_types');
  const unsubscribe = await service.listen('blobui-saved-state-changed', () => {});
  assert.deepEqual(errors, ['catalog warning']);
  assert.equal(calls, 1);
  assert.equal(typeof listener, 'function');
  unsubscribe();
  assert.equal(listener, null);
});

test('rejected metadata requests report the same error occurrence to callbacks and callers', async () => {
  const errors = [];
  const service = serviceFor({ core: { invoke: async () => { throw 'metadata unavailable'; } } }, (message, error) => errors.push({ message, error }));
  let caught;
  try { await service.invoke('discover_calendar_colors'); }
  catch (error) { caught = error; }
  assert.match(caught.message, /metadata unavailable/);
  assert.deepEqual(errors.map(item => item.message), ['metadata unavailable']);
  assert.equal(errors[0].error, caught);
});

test('separate native throws with the same string get separate error identities', async () => {
  const errors = [];
  const service = serviceFor({ core: { invoke: async () => { throw 'metadata unavailable'; } } }, (message, error) => errors.push({ message, error }));
  const caught = [];
  for (let i = 0; i < 2; i++) {
    try { await service.invoke('discover_calendar_colors'); }
    catch (error) { caught.push(error); }
  }
  assert.equal(caught.length, 2);
  assert.notEqual(caught[0], caught[1]);
  assert.equal(errors.length, 2);
  assert.equal(errors[0].error, caught[0]);
  assert.equal(errors[1].error, caught[1]);
});

test('missing Tauri bridge rejects invocations and event subscriptions become no-ops', async () => {
  const service = serviceFor(null);
  await assert.rejects(service.invoke('get_app_state'), /bridge is unavailable/);
  assert.equal(typeof await service.listen('event', () => {}), 'function');
});

test('disposing the native service ignores events and cancels pending metadata results', async () => {
  let handler, resolveRead;
  const errors = [];
  const service = serviceFor({
    core: { invoke() { return new Promise(resolve => { resolveRead = resolve; }); } },
    event: { async listen(name, callback) { handler = callback; return () => { handler = null; }; } },
  }, message => errors.push(message));
  let calls = 0;
  const unsubscribe = await service.listen('blobui-saved-state-changed', () => { calls++; });
  const pending = service.invoke('discover_calendar_colors');
  service.dispose();
  handler({});
  resolveRead({ types: [], views: [] });
  await assert.rejects(pending, /service is closed/);
  assert.equal(service.metadataPending.size, 0);
  assert.equal(calls, 0);
  assert.deepEqual(errors, []);
  unsubscribe();
});
