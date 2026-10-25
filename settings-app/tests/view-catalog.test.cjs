const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const source = fs.readFileSync(path.join(__dirname, '../ui/view-catalog.js'), 'utf8');
const context = {};
vm.runInNewContext(source, context, { filename: 'view-catalog.js' });
const ViewCatalog = context.BlobUI.ViewCatalog;

function view({ spaceId = 'space-a', objectId, blockId, viewId = 'view-a', kind, name = 'Board', space = 'Work', sourceSignature }) {
  return {
    target: { spaceId, objectId, blockId, viewId },
    name, space, source: 'Tasks', kind, sourceSignature, properties: [], unsupported: null,
  };
}

test('canonical query replaces its semantically matching inline alias while retaining exact targets', () => {
  const inline = view({ objectId: 'parent-doc', blockId: 'embed-block', kind: 'Inline / type view', sourceSignature: 'source-hash-a' });
  const canonical = view({ objectId: 'query-a', blockId: 'query-block', kind: 'Query', sourceSignature: 'source-hash-a' });
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

test('views without signatures only merge by exact target, regardless of display name', () => {
  const views = [
    view({ objectId: 'query-a', blockId: 'block-a', kind: 'Query' }),
    view({ objectId: 'query-a', blockId: 'block-b', kind: 'Inline / type view' }),
    view({ objectId: 'query-a', blockId: 'block-a', spaceId: 'space-b', kind: 'Query' }),
  ];

  const result = ViewCatalog.dedupeViews(views);

  assert.equal(result.views.length, 3);
  assert.equal(result.spacesById.size, 2);
  assert.equal(result.spacesById.get('space-a'), 'Work');
  assert.equal(result.spacesById.get('space-b'), 'Work');
});

test('saved definitions for every exact alias remain discoverable without coalescing tokens', () => {
  const inline = view({ objectId: 'parent-doc', blockId: 'embed-block', kind: 'Inline / type view', sourceSignature: 'source-hash-a' });
  const canonical = view({ objectId: 'query-a', blockId: 'query-block', kind: 'Query', sourceSignature: 'source-hash-a' });
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

test('different query definitions and spaces do not merge even when names match', () => {
  const views = [
    view({ objectId: 'parent-a', blockId: 'embed-a', kind: 'Inline / type view', sourceSignature: 'filters-estimate-gt-3' }),
    view({ objectId: 'parent-b', blockId: 'embed-b', kind: 'Inline / type view', sourceSignature: 'filters-estimate-gt-7' }),
    view({ spaceId: 'space-b', objectId: 'query-a', blockId: 'canonical', kind: 'Query', sourceSignature: 'filters-estimate-gt-3' }),
  ];

  const result = ViewCatalog.dedupeViews(views);

  assert.equal(result.views.length, 3);
});

test('matching native source signatures can merge different owning documents within one space', () => {
  const inline = view({ objectId: 'parent-doc', blockId: 'embed-block', kind: 'Inline / type view', sourceSignature: 'query-source-hash' });
  const canonical = view({ objectId: 'query-a', blockId: 'query-block', kind: 'Query', sourceSignature: 'query-source-hash' });

  const result = ViewCatalog.dedupeViews([inline, canonical]);

  assert.equal(result.views.length, 1);
  assert.equal(result.views[0].kind, 'Query');
  assert.deepEqual(Array.from(result.views[0].aliasTargets, target => target.objectId), ['query-a', 'parent-doc']);
});
