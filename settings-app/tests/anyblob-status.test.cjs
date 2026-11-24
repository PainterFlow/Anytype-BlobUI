const test=require('node:test');
const assert=require('node:assert/strict');

test('successful Anyblob refresh clears only the stale save warning',async()=>{
  const {SAVE_BUSY_STATUS,clearStaleSaveStatus}=await import('../ui/anyblob-status.js');
  assert.equal(clearStaleSaveStatus(SAVE_BUSY_STATUS),'');
  assert.equal(clearStaleSaveStatus('Could not save Anyblob.'),'Could not save Anyblob.');
  assert.equal(clearStaleSaveStatus('Remote access key copied.'),'Remote access key copied.');
});
