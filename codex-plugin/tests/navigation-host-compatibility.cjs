const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const readHost = require('./host-archive.cjs');
function probe(archive, fixturePath = path.resolve(__dirname, '../../.test-run/navigation/source-fixtures.json')) {
  const assets = readHost(archive, /^(rolldown-runtime-428|app-shared-|local-conversation-(external-resources|summary-panel-sources-model|thread)-|use-local-conversation-sources-).*\.js$/);
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
  // Run the actual schema and bundled Zod. No app imports, state, network,
  // filesystem, timers or transports are available inside this VM.
  const schema = vm.createContext({ URL, TextEncoder, TextDecoder });
  const runtime = asset('rolldown-runtime-428');
  Object.assign(schema, vm.runInContext(`(()=>{${runtime.slice(0, runtime.indexOf('export{'))};return {e:d,t:o,n:u,r:c,i:s}})()`, schema));
  const shared = asset('app-shared-');
  assert(shared.includes('url:q().refine(Wut)'));
  vm.runInContext(shared.replace(/import\{[^}]*\}from"[^"]+";/g, '')
    .replace(/export\{[^}]+\};?/g, '').replace(/import\.meta/g, '({})'), schema, { timeout: 3000 });
  vm.runInContext('Yut()', schema, { timeout: 1000 });
  const fixtures = JSON.parse(fs.readFileSync(fixturePath, 'utf8').replace(/^\uFEFF/, ''));
  assert.equal(schema.Uut(fixtures.read._meta).status, 'valid');
  const invalid = structuredClone(fixtures.read._meta);
  invalid['openai/resourceActivities'].resources[0].url = fixtures.read.content[1].uri;
  assert.equal(schema.Uut(invalid).status, 'invalid');
  const external = vm.createContext({ URL, Set, Map, WeakMap,
    i: value => schema.Uut(value), y: items => items, m: () => false,
    ne: () => null, r: name => name, ee: () => null,
  });
  load(external, asset('local-conversation-external-resources-'), 'function ae(', 'var Z,Q;');
  vm.runInContext('D=[]; F=new Set(); Z=["provided","read","created","updated"];', external);
  function call(id, result, server = 'anyblob') {
    return { id, type: 'mcpToolCall', server, tool: 'read_object', status: 'completed', result };
  }
  const turns = items => [{ items, params: { input: [] } }];
  const collect = items => external.L(turns(items)).filter(external.R);
  const calls = [call('first', fixtures.read), call('repeat', fixtures.read),
    call('duplicate', fixtures.duplicateTitle), call('rename', fixtures.renamed), call('create', fixtures.created)];
  const sources = collect(calls);
  assert.equal(sources.length, 3, 'Deduplicate by space/object, including confirmed creations');
  assert(sources.some(s => s.title === 'Same title · Space b'));
  assert(sources.some(s => s.title === 'Renamed · Space a'), 'Later authorized titles replace earlier titles');
  for (const key of ['unknown', 'dry', 'search']) assert.equal(collect([call(key, fixtures[key])]).length, 0, key);
  assert.equal(collect([{ ...call('failed', fixtures.read), status: 'failed' }]).length, 0);
  assert.equal(collect([{ ...call('denied', fixtures.read), error: { message: 'Denied' } }]).length, 0);
  assert.equal(collect([]).length, 0, 'Separate chats must not inherit sources');
  assert.equal(JSON.stringify(collect(JSON.parse(JSON.stringify(calls)))), JSON.stringify(sources));
  assert.equal(collect([call('renamed', fixtures.renamed)])[0].title, 'Renamed · Space a');
  const tools = vm.createContext({ Set, Map,
    V: () => new Map(), j: items => items, h: () => null, d: () => false, U: name => name,
    m: () => false, A: (_turn, item) => ({ callId: item.id, result: item.result, completed: true }),
    N: () => false, E: () => null, l: value => value, B: () => 'Read object', s: name => name, W: () => null,
  });
  load(tools, asset('local-conversation-summary-panel-sources-model-'), 'function K(', 'function q(');
  load(tools, asset('local-conversation-summary-panel-sources-model-'), 'function Y(', 'var Z;');
  const providers = new Set(sources.map(s => s.provider.id));
  const remaining = tools.K(turns([...calls, call('other', {}, 'unrelated')]), [], {}, new Set(), [], providers, false, []);
  assert.equal(remaining.length, 1, 'Replace generic Anyblob while preserving unrelated tool sources');
  assert.equal(remaining[0].name, 'unrelated');
  const connectedCall = { ...call('connected', fixtures.read), appContext: { connectorId: 'connector-anyblob', appName: 'Anyblob' } };
  const connectedSources = collect([connectedCall]);
  assert.equal(connectedSources[0].provider.id, 'connector-anyblob');
  assert.equal(tools.K(turns([connectedCall]), [], {}, new Set(), [], new Set(['connector-anyblob']), false, []).length, 0);
  const jsx = (type, props) => ({ type, props });
  const cache = n => Array(n).fill(Symbol.for('react.memo_cache_sentinel'));
  const nav = vm.createContext({ Set, Symbol, _k: { c: cache }, vk: { jsx },
    I: () => ({ formatMessage: () => 'Sources' }), bv: () => null,
    $: { Sources: 'Sources', SourceRow: 'SourceRow' }, rx: 'ExternalResource',
    $i: 'icon', Vs: 'logo', Xg: () => null,
  });
  load(nav, asset('local-conversation-thread-'), 'function fk(', 'var _k,vk;');
  const tree = nav.fk({ fileSources: [], externalSources: sources, toolSources: [], hasWebSearch: false,
    hostId: 'fixture', webMcpSourceHostnames: [] });
  assert.equal(tree.props.sources.length, 3);
  const entry = tree.props.children(tree.props.sources[0]);
  const row = nav.pk(entry.props);
  assert.equal(row.type, 'ExternalResource');
  assert.equal(row.props.resource.url, sources[0].url);
  let opened;
  const opener = vm.createContext({ Symbol, J: { c: cache }, Y: { jsx },
    f: () => ({}), O: {}, n: () => ({}), i: () => null,
    re: 'signal', K: 'title', d: key => key === 'title' ? { data: null } : null,
    H: { SourceRow: 'SourceRow', SourceDetails: 'SourceDetails' }, B: 'icon',
    ae: () => false, h: options => { opened = options.href; },
  });
  load(opener, asset('use-local-conversation-sources-'), 'function ue(', 'var J,Y;');
  const openRow = opener.ue({ hostId: 'fixture', presentation: 'summary', resource: sources[0] });
  openRow.props.onOpen({});
  assert.equal(opened, sources[0].url, 'Actual row click dispatches the Anytype HTTPS universal link');
  assert(opened.startsWith('https://object.any.coop/'));
  console.log(JSON.stringify({ decision: 'PASS', individualSources: sources.length,
    productionSerializer: true, actualMetadataParser: true, deduplication: true,
    unrelatedSourcesPreserved: true, resourceOpenDispatch: opened,
    reload: 'Serialized synthetic turns; durable desktop persistence remains unverified',
    limitations: 'Host controls ordering and title precedence. Click opens Anytype, not the MCP reader. No live app automated.' }, null, 2));
}
module.exports = probe;
if (require.main === module) probe(process.argv[2], process.argv[3]);
