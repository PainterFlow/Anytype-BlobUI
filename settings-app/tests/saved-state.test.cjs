const test = require('node:test');
const assert = require('node:assert/strict');
const vm = require('node:vm');
const fs = require('node:fs');
const sandbox = { window: {} };
vm.runInNewContext(fs.readFileSync(require('node:path').join(__dirname,'../ui/saved-state.js'),'utf8'), sandbox);
const { merge } = sandbox.window.BlobUI.SavedState;
const plain = x => JSON.parse(JSON.stringify(x));
const target = id => ({spaceId:'s',objectId:'o',blockId:'b',viewId:id});
const definition = (id, value) => ({target:target(id),tokens:[{kind:'number',value:String(value)}]});
test('three-way settings merge keeps independent edits and saved values win conflicts', () => {
  const warnings=[];
  assert.deepEqual(plain(merge({a:1,b:1,c:1},{a:2,b:1,c:2},{a:1,b:3,c:3},p=>warnings.push(p))),{a:2,b:3,c:3});
  assert.deepEqual(warnings,['Settings.c']);
});
test('calculations merge by full target, with atomic tokens and deletion conflicts', () => {
  const base={version:2,display:'balanced',definitions:[definition('a',1),definition('b',1),definition('c',1)]};
  const local={...base,display:'spacious',definitions:[definition('a',2),definition('b',2),definition('c',2)]};
  const remote={...base,definitions:[definition('a',3),definition('b',1),definition('d',4)]};
  const warnings=[];
  const result=plain(merge(base,local,remote,p=>warnings.push(p),'Calculations'));
  assert.equal(result.display,'spacious');
  assert.deepEqual(result.definitions,[definition('a',3),definition('b',2),definition('d',4)]);
  assert.equal(warnings.length,2);
});
test('calendar defaults and overrides reconcile independently by stable targets', () => {
  const type={spaceId:'s',typeKey:'task',default:{mode:'inherit'},overrides:[{target:target('v'),rule:{mode:'property',key:'status'}}]};
  const base={version:1,types:[type]};
  const local={version:1,types:[{...type,default:{mode:'none'}}]};
  const remote={version:1,types:[{...type,overrides:[{target:target('v'),rule:{mode:'property',key:'priority'}}]}]};
  const result=plain(merge(base,local,remote,()=>assert.fail('Unrelated changes are not conflicts'),'Calendar'));
  assert.equal(result.types[0].default.mode,'none');
  assert.equal(result.types[0].overrides[0].rule.key,'priority');
  const warnings=[];
  assert.deepEqual(plain(merge(base,local,{version:1,types:[]},p=>warnings.push(p),'Calendar')).types,[]);
  assert.equal(warnings.length,1);
});

test('new calendar defaults and view overrides are independent additions',()=>{
  const base={version:1,types:[]};
  const local={version:1,types:[{spaceId:'s',typeKey:'task',default:{mode:'none'},overrides:[]}]};
  const remote={version:1,types:[{spaceId:'s',typeKey:'task',default:{mode:'inherit'},overrides:[{target:target('v'),rule:{mode:'property',key:'status'}}]}]};
  const result=plain(merge(base,local,remote,()=>assert.fail('Independent rules should merge'),'Calendar'));
  assert.equal(result.types[0].default.mode,'none');assert.equal(result.types[0].overrides.length,1);
});

test('refresh controller coalesces events during a read into one follow-up', async () => {
  const { createRefreshController } = sandbox.window.BlobUI.SavedState;
  let resolveFirst;
  let reads = 0;
  const controller = createRefreshController({ refresh: () => {
    reads++;
    if (reads === 1) return new Promise(resolve => { resolveFirst = resolve; });
  } });
  controller.setReady();
  const first = controller.request();
  await Promise.resolve();
  controller.request();
  controller.request();
  assert.equal(reads, 1);
  resolveFirst();
  await first;
  await new Promise(resolve => setImmediate(resolve));
  assert.equal(reads, 2);
  assert.equal(controller.pending, null);
  assert.equal(controller.queued, false);
});

test('refresh controller queues while blocked and retries after a failed read', async () => {
  const { createRefreshController } = sandbox.window.BlobUI.SavedState;
  let blocked = true;
  let fail = true;
  let reads = 0;
  const controller = createRefreshController({ isBlocked: () => blocked, refresh: () => {
    reads++;
    if (fail) throw new Error('snapshot unavailable');
  } });
  controller.request();
  await controller.setReady();
  assert.equal(reads, 0);
  assert.equal(controller.queued, true);
  blocked = false;
  await assert.rejects(controller.flush(), /snapshot unavailable/);
  assert.equal(controller.pending, null);
  fail = false;
  await controller.request();
  assert.equal(reads, 2);
  assert.equal(controller.queued, false);
});

test('refresh controller marks an in-flight result stale after invalidation', async () => {
  const { createRefreshController } = sandbox.window.BlobUI.SavedState;
  let resolveRead;
  let stale = null;
  const controller = createRefreshController({ refresh: context => new Promise(resolve => {
    resolveRead = () => { stale = !context.isCurrent(); resolve(); };
  }) });
  controller.setReady();
  const read = controller.request();
  await Promise.resolve();
  controller.invalidate();
  resolveRead();
  await read;
  assert.equal(stale, true);
  assert.equal(controller.generation, 1);
});
