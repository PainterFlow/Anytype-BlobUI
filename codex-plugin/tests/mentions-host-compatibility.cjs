// Read the installed host without registering a plugin or touching personal data.
// Exercise its real mention dispatcher against an isolated stdio fixture.
const fs = require('node:fs');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const { spawn } = require('node:child_process');
const readline = require('node:readline');

if (process.argv[2] === '--fixture') {
  readline.createInterface({ input: process.stdin }).on('line', line => {
    const request = JSON.parse(line);
    const threadId = request.params._meta?.threadId;
    const objects = { 'project-a-chat': 'Only A', 'project-b-chat': 'Only B' };
    const result = objects[threadId]
      ? { items: [{ type: 'resource_link', uri: `anyblob://spaces/fixture/objects/${threadId}`, name: objects[threadId] }] }
      : { items: [], denied: 'Missing verified chat identity' };
    console.log(JSON.stringify({ jsonrpc: '2.0', id: request.id, result: { content: [], structuredContent: result } }));
  });
} else {
  main().catch(error => { console.error(error); process.exitCode = 1; });
}

async function main() {
  const archive = process.argv[2];
  assert(archive, 'Pass the installed desktop app.asar path.');
  const file = fs.openSync(archive, 'r');
  let source;
  let asset;
  try {
    const header = Buffer.alloc(16);
    fs.readSync(file, header, 0, 16, 0);
    const data = Buffer.alloc(header.readUInt32LE(12));
    fs.readSync(file, data, 0, data.length, 16);
    const base = 8 + header.readUInt32LE(4);
    function visit(files, prefix = '') {
      for (const [name, entry] of Object.entries(files)) {
        const path = `${prefix}/${name}`;
        if (entry.files) visit(entry.files, path);
        else if (!entry.unpacked && /\/webview\/assets\/app-initial-.*\.js$/.test(path)) {
          const bytes = Buffer.alloc(entry.size);
          fs.readSync(file, bytes, 0, bytes.length, base + Number(entry.offset));
          const text = bytes.toString('utf8');
          if (text.includes('async function sIn(')) { source = text; asset = path; }
        }
      }
    }
    visit(JSON.parse(data).files);
  } finally { fs.closeSync(file); }
  assert(source, 'Host changed. Inspect its dispatcher before updating this version-specific probe.');
  assert(source.includes('vSn=`openai/mentions`'));
  const start = source.indexOf('async function sIn(');
  const end = source.indexOf('async function cIn(', start);
  assert(end > start);
  const child = spawn(process.execPath, [__filename, '--fixture'], { stdio: ['pipe', 'pipe', 'inherit'], windowsHide: true });
  const pending = new Map();
  const deadline = setTimeout(() => { child.kill(); console.error('Fixture timed out.'); process.exitCode = 1; }, 10000);
  let sequence = 0;
  const lines = readline.createInterface({ input: child.stdout });
  lines.on('line', line => {
    const response = JSON.parse(line);
    pending.get(response.id)?.(response.result);
    pending.delete(response.id);
  });
  const observed = [];
  function call(options, query) {
    const request = { jsonrpc: '2.0', id: ++sequence, method: 'tools/call', params: { name: 'search_mentions', arguments: { query } } };
    if (options.threadId) request.params._meta = { threadId: options.threadId };
    observed.push(request);
    return new Promise(resolve => {
      pending.set(request.id, resolve);
      child.stdin.write(JSON.stringify(request) + '\n');
    });
  }
  const parser = { safeParse: data => ({ success: true, data }) };
  const context = vm.createContext({
    Promise, Error, Tr: { debug() {}, warning() {} }, ZNn: () => ({}),
    bIn: parser, _In: parser,
    // Capture exactly what the installed dispatcher supplies to its tool caller.
    cIn: (_scope, _host, _server, _tool, args, options) => call(options, args.query),
  });
  vm.runInContext(source.slice(start, end), context, { timeout: 1000 });
  try {
    const providers = [{ mentionSearchTool: 'search_mentions', server: 'fixture', usesLegacySearchArguments: false }];
    const results = await Promise.all(['project-a-chat', 'project-b-chat'].map(threadId =>
      context.sIn({ activeThreadId: threadId }, 'local', providers, { query: '', threadId })));
    assert.equal(observed.length, 2);
    assert(observed.every(request => !request.params._meta?.threadId));
    assert(results.every(result => result[0].items.length === 0));
    // Control cases prove that the fixture distinguishes projects when identified.
    assert.equal((await call({ threadId: 'project-a-chat' }, '')).structuredContent.items[0].name, 'Only A');
    assert.equal((await call({ threadId: 'project-b-chat' }, '')).structuredContent.items[0].name, 'Only B');
    console.log(JSON.stringify({ asset, extensionRecognized: true, concurrentSearches: 2, activeChatForwarded: false, projectControlsPassed: true, decision: 'BLOCKED' }, null, 2));
  } finally {
    clearTimeout(deadline);
    child.stdin.end();
    lines.close();
    child.kill();
  }
}
