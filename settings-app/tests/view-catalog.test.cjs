const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../ui/view-catalog.js'), 'utf8');
const context = {};
vm.runInNewContext(source, context, { filename: 'view-catalog.js' });
const ViewCatalog = context.BlobUI.ViewCatalog;

function view({ spaceId = 'space-a', objectId, blockId, viewId = 'view-a', kind, name = 'Board', space = 'Work' }) {
  return {
    target: { spaceId, objectId, blockId, viewId },
    name, space, source: 'Tasks', kind, properties: [], unsupported: null,
  };
}

test('canonical list view replaces its inline duplicate while retaining exact target aliases', () => {
  const inline = view({ objectId: 'query-a', blockId: 'embed-block', kind: 'Inline / type view' });
  const canonical = view({ objectId: 'query-a', blockId: 'query-block', kind: 'Query' });
  const input = [inline, canonical];

  const result = ViewCatalog.dedupeViews(input);

  assert.equal(result.views.length, 1);
  const [representative] = result.views;
  assert.equal(representative.kind, 'Query');
  assert.equal(ViewCatalog.targetIdentity(representative.target), ViewCatalog.targetIdentity(canonical.target));
  assert.deepEqual(Array.from(representative.aliasTargets, target => target.blockId), ['query-block', 'embed-block']);
  assert.equal(Object.prototype.propertyIsEnumerable.call(representative, 'aliasTargets'), false);
  assert.deepEqual(input, [inline, canonical], 'deduplication leaves API records untouched');
});

test('same display names do not merge across query identities or spaces', () => {
  const views = [
    view({ objectId: 'query-a', blockId: 'block-a', kind: 'Query' }),
    view({ objectId: 'query-b', blockId: 'block-b', kind: 'Query' }),
    view({ spaceId: 'space-b', objectId: 'query-a', blockId: 'block-c', kind: 'Query' }),
  ];

  const result = ViewCatalog.dedupeViews(views);

  assert.equal(result.views.length, 3);
  assert.equal(result.spacesById.size, 2);
  assert.equal(result.spacesById.get('space-a'), 'Work');
  assert.equal(result.spacesById.get('space-b'), 'Work');
});

test('saved definitions for every exact alias remain discoverable without coalescing tokens', () => {
  const inline = view({ objectId: 'query-a', blockId: 'embed-block', kind: 'Inline / type view' });
  const canonical = view({ objectId: 'query-a', blockId: 'query-block', kind: 'Query' });
  const representative = ViewCatalog.dedupeViews([inline, canonical]).views[0];
  const canonicalDefinition = { target: canonical.target, tokens: [{ kind: 'number', value: '3' }] };
  const inlineDefinition = { target: inline.target, tokens: [{ kind: 'number', value: '9' }] };
  const otherDefinition = {
    target: { ...inline.target, objectId: 'query-b' },
    tokens: [{ kind: 'number', value: '7' }],
  };

  const definitions = ViewCatalog.definitionsForView(representative, [canonicalDefinition, inlineDefinition, otherDefinition]);

  assert.deepEqual(definitions, [canonicalDefinition, inlineDefinition]);
  assert.deepEqual(definitions.map(item => item.tokens[0].value), ['3', '9']);
  assert.equal(ViewCatalog.hasDefinition(representative, [inlineDefinition]), true);
  assert.equal(ViewCatalog.findViewForTarget([representative], inline.target), representative);
  assert.equal(ViewCatalog.findViewForTarget([representative], otherDefinition.target), null);
});

test('views without complete target identity remain separate', () => {
  const first = view({ objectId: 'query-a', blockId: 'block-a', kind: 'Inline / type view' });
  const incomplete = { ...first, target: { ...first.target, blockId: '' } };

  const result = ViewCatalog.dedupeViews([first, incomplete]);

  assert.equal(result.views.length, 2);
});
