const assert = require('node:assert/strict');
const vm = require('node:vm');
const readHost = require('./host-archive.cjs');

function probe(archive) {
  const assets = readHost(archive, /^(app-shared-|local-conversation-(external-resources|summary-panel-sources-model|thread)-|use-local-conversation-sources-).*\.js$/);
  function asset(prefix) {
    const match = [...assets].find(([name]) => name.startsWith(prefix));
    assert(match, `Host changed. Missing ${prefix}`);
    return match[1];
  }
  function load(context, source, start, end) {
    const a = source.indexOf(start), b = source.indexOf(end, a);
    assert(a >= 0 && b > a, `Host changed. Inspect ${start}`);
    vm.runInContext(source.slice(a, b), context, { timeout: 1000 });
  }
  const external = vm.createContext({ URL, Set, Map, WeakMap,
    // Isolate app discovery and the metadata parser. No host transport or state.
    i: () => null, y: items => items, m: () => false, ne: () => null,
    r: name => name, ee: () => null,
  });
  const extSource = asset('local-conversation-external-resources-');
  load(external, extSource, 'function ae(', 'var Z,Q;');
  vm.runInContext('D=[]; F=new Set(); Z=["provided","read","created","updated"];', external);
  // Set up only initialized constants, never execute the module initializer.
  const uri = 'anyblob://objects/space/object';
  // The real resourceActivities schema uses this exact URL refinement.
  const shared = asset('app-shared-');
  assert(shared.includes('url:q().refine(Wut)'));
  assert(shared.includes('"openai/resourceActivities":J({version:Im(1),resources:Em(Kut)})'));
  load(external, shared, 'function Wut(', 'var Gut,');
  assert.equal(external.Wut(uri), false);
  assert.equal(external.Wut('https://example.com/object'), true);
  assert.equal(external.q(uri), null);
  assert.equal(external.K({ activities: ['read'], title: 'Same title', url: uri }), null);
  assert(external.K({ activities: ['read'], title: 'Control', url: 'https://example.com/object' }));
  const link = { type: 'resource_link', uri, name: 'Same title · Space' };
  function call(id, outcome = 'confirmed') {
    return { id, type: 'mcpToolCall', server: 'anyblob', tool: 'read_object', status: 'completed',
      result: { content: [link], structuredContent: { outcome, receipt: { id: 'object', name: 'Same title' } } } };
  }
  const turns = items => [{ items, params: { input: [] } }];
  assert.equal(external.L(turns([call('one'), call('two')])).length, 0);
  assert.equal(external.L(JSON.parse(JSON.stringify(turns([call('one')])))).length, 0);
  assert.equal(external.L(turns([])).length, 0);

  const tools = vm.createContext({ Set, Map,
    V: () => new Map(), j: items => items, h: () => null, d: () => false, U: name => name,
    m: () => false, A: (_turn, item) => ({ callId: item.id, result: item.result, completed: true }),
    N: () => false, E: () => null, l: value => value, B: () => 'Read object',
    s: name => name, W: () => null,
  });
  load(tools, asset('local-conversation-summary-panel-sources-model-'), 'function K(', 'function q(');
  load(tools, asset('local-conversation-summary-panel-sources-model-'), 'function Y(', 'var Z;');
  const collect = items => tools.K(turns(items), [], {}, new Set(), [], new Set(), false, []);
  const collected = collect([call('one'), call('two')]);
  assert.equal(collected.length, 1);
  assert.equal(collected[0].id, 'mcp-server:anyblob');
  assert.equal(collected[0].name, 'anyblob');
  assert.equal(collect([call('unknown', 'unknown')]).length, 1, 'Host does not inspect Anyblob mutation outcomes');
  assert.equal(collect([]).length, 0, 'Synthetic chats must stay isolated');
  assert.equal(collect(JSON.parse(JSON.stringify([call('one')])))[0].id, collected[0].id);

  const jsx = (type, props) => ({ type, props });
  const nav = vm.createContext({ Set, Symbol,
    _k: { c: n => Array(n).fill(Symbol.for('react.memo_cache_sentinel')) },
    vk: { jsx }, I: () => ({ formatMessage: () => 'Sources' }), bv: () => null,
    $: { Sources: 'Sources', SourceRow: 'SourceRow' }, $i: 'icon', Vs: 'logo', Xg: () => null,
  });
  load(nav, asset('local-conversation-thread-'), 'function fk(', 'var _k,vk;');
  const tree = nav.fk({ fileSources: [], externalSources: [], toolSources: collected,
    hasWebSearch: false, hostId: 'fixture', webMcpSourceHostnames: [] });
  assert.equal(tree.props.sources.length, 1);
  const entry = tree.props.children(tree.props.sources[0]);
  const row = nav.pk(entry.props);
  assert.equal(row.props.label, 'anyblob');
  assert.equal(row.props.onOpen, undefined, 'Generic connector row cannot open an object resource');
  console.log(JSON.stringify({ decision: 'BLOCKED', resourceLinksProduceIndividualSources: false,
    canonicalUriAccepted: false, genericSourceOpensObject: false,
    reload: 'Serialized synthetic turns only; no durable host persistence verified',
    taskRow: 'No supported registration, association persistence or goal context interface verified',
    scope: 'Actual external and tool collectors and navigation components; isolated discovery, metadata parser, React element factory and transport' }, null, 2));
}
module.exports = probe;
if (require.main === module) probe(process.argv[2]);
