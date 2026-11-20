const { test } = require('node:test');
const assert = require('node:assert/strict');

async function fixture() {
  const { createAnyblobDraft } = await import('../ui/anyblob-draft.js');
  const draft = createAnyblobDraft(), calls = [];
  let failToggle = false;
  draft.init({ onChange() {}, async invoke(command, args) {
    calls.push({ command, args });
    if (command === 'anyblob_status') return { enabled: true };
    if (command === 'configure_anyblob' && failToggle) throw Error('Anyblob rejected');
    if (command === 'configure_anyblob') return { enabled: args.enabled };
    return {};
  } });
  await draft.load(); draft.setLocked(false); calls.length = 0;
  return { draft, calls, rejectToggle() { failToggle = true; } };
}

test('integration edits remain drafts across reload and Undo restores the saved values', async () => {
  const { draft, calls } = await fixture();
  draft.change({ enabled: false });
  assert.equal(calls.length, 0);
  await draft.load();
  assert.equal(draft.getSnapshot().enabled, false);
  assert.equal(draft.isDirty(), true);
  draft.discard();
  assert.equal(draft.getSnapshot().enabled, true);
  assert.equal(draft.isDirty(), false);
});

test('Save persists Anyblob enablement', async () => {
  const { draft, calls } = await fixture();
  draft.change({ enabled: false });
  await draft.save();
  assert.deepEqual(calls, [
    { command: 'configure_anyblob', args: { enabled: false } },
  ]);
  assert.equal(draft.isDirty(), false);
});

test('failed Anyblob configuration preserves the draft for Undo', async () => {
  const { draft, rejectToggle } = await fixture();
  draft.change({ enabled: false });
  rejectToggle();
  await assert.rejects(draft.save(), /Anyblob rejected/);
  assert.equal(draft.isDirty(), true);
  assert.equal(draft.getSnapshot().enabled, false);
  draft.discard();
  assert.equal(draft.getSnapshot().enabled, true);
});
