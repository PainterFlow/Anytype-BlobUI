const { test } = require('node:test');
const assert = require('node:assert/strict');

async function fixture() {
  const { createAnyblobDraft } = await import('../ui/anyblob-draft.js');
  const draft = createAnyblobDraft(), calls = [];
  let failToggle = false;
  draft.init({ onChange() {}, async invoke(command, args) {
    calls.push({ command, args });
    if (command === 'anyblob_status') return { enabled: true, localProjectsFolder: 'C:\\projects' };
    if (command === 'configure_anyblob' && failToggle) throw Error('Anyblob rejected');
    if (command === 'configure_anyblob') return { enabled: args.enabled };
    if (command === 'set_anyblob_projects_folder') return { localProjectsFolder: args.folder };
    return {};
  } });
  await draft.load(); draft.setLocked(false); calls.length = 0;
  return { draft, calls, rejectToggle() { failToggle = true; } };
}

test('integration edits remain drafts across reload and Undo restores the saved values', async () => {
  const { draft, calls } = await fixture();
  draft.change({ folder: 'C:\\new', enabled: false });
  assert.equal(calls.length, 0);
  await draft.load();
  assert.equal(draft.getSnapshot().folder, 'C:\\new');
  assert.equal(draft.getSnapshot().enabled, false);
  assert.equal(draft.isDirty(), true);
  draft.discard();
  assert.equal(draft.getSnapshot().folder, 'C:\\projects');
  assert.equal(draft.getSnapshot().enabled, true);
  assert.equal(draft.isDirty(), false);
});

test('Save persists Anyblob enablement and the local projects folder', async () => {
  const { draft, calls } = await fixture();
  draft.change({ folder: 'C:\\new', enabled: false });
  await draft.save();
  assert.deepEqual(calls, [
    { command: 'configure_anyblob', args: { enabled: false } },
    { command: 'set_anyblob_projects_folder', args: { folder: 'C:\\new' } },
  ]);
  assert.equal(draft.isDirty(), false);
});

test('failed Anyblob configuration preserves the draft for Undo', async () => {
  const { draft, rejectToggle } = await fixture();
  draft.change({ folder: 'C:\\new', enabled: false });
  rejectToggle();
  await assert.rejects(draft.save(), /Anyblob rejected/);
  assert.equal(draft.isDirty(), true);
  assert.equal(draft.getSnapshot().folder, 'C:\\new');
  assert.equal(draft.getSnapshot().enabled, false);
  draft.discard();
  assert.equal(draft.getSnapshot().folder, 'C:\\projects');
  assert.equal(draft.getSnapshot().enabled, true);
});