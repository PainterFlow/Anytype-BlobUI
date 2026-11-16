const { test } = require('node:test');
const assert = require('node:assert/strict');

async function fixture() {
  const { createAnyblobDraft } = await import('../ui/anyblob-draft.js');
  const draft = createAnyblobDraft(), calls = [];
  let failTunnel = false;
  draft.init({ onChange() {}, async invoke(command, args) {
    calls.push({ command, args });
    if (command === 'anyblob_status') return { enabled: true, localProjectsFolder: 'C:\\projects' };
    if (command === 'secure_mcp_tunnel_status') return { enabled: false, tunnelId: '' };
    if (command === 'set_anyblob_projects_folder') return { localProjectsFolder: args.folder };
    if (command === 'configure_secure_mcp_tunnel' && failTunnel) throw Error('Tunnel rejected');
    return {};
  } });
  await draft.load(); draft.setLocked(false); calls.length = 0;
  return { draft, calls, rejectTunnel() { failTunnel = true; } };
}

test('integration edits remain drafts across reload and Undo restores the saved values', async () => {
  const { draft, calls } = await fixture();
  draft.change({ folder: 'C:\\new', tunnelEnabled: true, tunnelId: 'new-id', runtimeApiKey: 'secret' });
  assert.equal(calls.length, 0);
  await draft.load();
  assert.equal(draft.getSnapshot().folder, 'C:\\new');
  assert.equal(draft.getSnapshot().runtimeApiKey, 'secret');
  assert.equal(draft.isDirty(), true);
  draft.discard();
  assert.equal(draft.getSnapshot().folder, 'C:\\projects');
  assert.equal(draft.getSnapshot().runtimeApiKey, '');
  assert.equal(draft.isDirty(), false);
});

test('Save persists folder and tunnel configuration without starting a process', async () => {
  const { draft, calls } = await fixture();
  draft.change({ folder: 'C:\\new', tunnelEnabled: true, tunnelId: ' tunnel ', runtimeApiKey: ' secret ' });
  await draft.save();
  assert.deepEqual(calls.map(call => call.command), ['set_anyblob_projects_folder', 'configure_secure_mcp_tunnel']);
  assert.deepEqual(calls[1].args, { enabled: true, tunnelId: 'tunnel', runtimeApiKey: 'secret', start: false });
  assert.equal(draft.isDirty(), false);
  assert.equal(draft.getSnapshot().runtimeApiKey, '');
});

test('failed tunnel save preserves its draft and Undo retains a successfully saved folder', async () => {
  const { draft, rejectTunnel } = await fixture();
  draft.change({ folder: 'C:\\new', tunnelEnabled: true, tunnelId: 'tunnel', runtimeApiKey: 'secret' });
  rejectTunnel();
  await assert.rejects(draft.save(), /Tunnel rejected/);
  assert.equal(draft.isDirty(), true);
  assert.equal(draft.getSnapshot().runtimeApiKey, 'secret');
  draft.discard();
  assert.equal(draft.getSnapshot().folder, 'C:\\new');
  assert.equal(draft.getSnapshot().tunnelEnabled, false);
});

test('disabling Anyblob clears the pending secret without writing before Save', async () => {
  const { draft, calls } = await fixture();
  draft.change({ tunnelEnabled: true, runtimeApiKey: 'secret' });
  draft.change({ enabled: false });
  assert.equal(draft.getSnapshot().runtimeApiKey, '');
  assert.equal(draft.getSnapshot().tunnelEnabled, false);
  assert.equal(calls.length, 0);
  draft.discard();
  assert.equal(draft.getSnapshot().enabled, true);
});
