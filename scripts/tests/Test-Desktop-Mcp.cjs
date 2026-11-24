const assert = require('node:assert/strict');
const path = require('node:path');
const { spawnSync } = require('node:child_process');

const executable = process.argv[2];
assert(executable, 'Pass the compiled BlobUI Settings executable.');

const messages = [
  {
    jsonrpc: '2.0',
    id: 1,
    method: 'initialize',
    params: {
      protocolVersion: '2025-11-25',
      capabilities: {},
      clientInfo: { name: 'desktop-mcp-fixture', version: '1' },
    },
  },
  { jsonrpc: '2.0', method: 'notifications/initialized' },
  { jsonrpc: '2.0', id: 2, method: 'tools/list' },
];
const fixture = path.resolve(__dirname, '../../.test-run/desktop-mcp');
const result = spawnSync(executable, ['mcp'], {
  encoding: 'utf8',
  timeout: 15000,
  windowsHide: true,
  env: {
    ...process.env,
    ANYBLOB_BRIDGE_INSTANCE: `desktop-mcp-${process.pid}`,
    ANYBLOB_CREDENTIAL_FILE: path.join(fixture, 'bridge.dpapi'),
  },
  input: `${messages.map(message => JSON.stringify(message)).join('\n')}\n`,
});

assert.ifError(result.error);
assert.equal(result.status, 0, result.stderr || `MCP process exited ${result.status}`);
const responses = result.stdout.trim().split(/\r?\n/).map(line => JSON.parse(line));
const initialized = responses.find(response => response.id === 1)?.result;
const listed = responses.find(response => response.id === 2)?.result;
assert.equal(initialized?.serverInfo?.name, 'anyblob');
assert.equal(initialized?.capabilities?.extensions?.['openai/mentions']?.searchTool, 'search_mentions');
assert(Array.isArray(listed?.tools));
const mentions = listed.tools.find(tool => tool.name === 'search_mentions');
assert(mentions, 'The desktop MCP server must expose search_mentions.');
assert.deepEqual(mentions._meta?.ui?.visibility, ['app']);
console.log(JSON.stringify({ server: initialized.serverInfo.name, tools: listed.tools.length, search_mentions: true }));
