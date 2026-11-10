const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const root = path.resolve(process.argv[2]);
const plugin = JSON.parse(fs.readFileSync(path.join(root, '.codex-plugin/plugin.json')));
const mcp = JSON.parse(fs.readFileSync(path.join(root, '.mcp.json')));
const portablePlugin = JSON.parse(fs.readFileSync(path.join(root, 'plugin.json')));
const portableMcp = JSON.parse(fs.readFileSync(path.join(root, 'mcp.json')));
assert.equal(plugin.name, 'anyblob');
assert.match(plugin.version, /^\d+\.\d+\.\d+$/);
assert.equal(plugin.version, JSON.parse(fs.readFileSync(path.resolve(__dirname, '../src/.codex-plugin/plugin.json'))).version);
assert.equal(plugin.interface.displayName, 'Anyblob');
assert.equal(plugin.requires_local_executor, true);
assert.equal(portablePlugin.$schema, 'https://agent-plugins.org/schemas/1.0.0/plugin.schema.json');
assert.equal(portablePlugin.name, plugin.name);
assert.equal(portablePlugin.version, plugin.version);
assert.equal(portablePlugin.extensions['com.openai'].requires_local_executor, true);
assert.deepEqual(portablePlugin.extensions['com.openai'].interface, plugin.interface);
assert.equal(portableMcp.$schema, 'https://agent-plugins.org/schemas/1.0.0/mcp.schema.json');
assert.deepEqual(Object.keys(portableMcp.mcpServers), ['anyblob']);
assert.deepEqual(portableMcp.mcpServers.anyblob, {
  type: 'stdio', command: './bin/blobui-mcp.exe', args: [],
});
const server = mcp.mcpServers.anyblob;
assert.deepEqual(Object.keys(mcp.mcpServers), ['anyblob']);
assert.equal(plugin.mcpServers, './.mcp.json');
assert.equal(server.command, 'cmd.exe');
assert.deepEqual(server.args, ['/d', '/c', '.\\bin\\blobui-mcp.exe']);
assert.equal(server.cwd, '.');
assert(server.tool_timeout_sec > 120);
assert.equal(server.env_vars, undefined);
assert.equal(server.env, undefined);
for (const relative of ['bin/blobui-mcp.exe', 'assets/icon.png', 'README.md']) assert(fs.statSync(path.join(root, relative)).size > 0);
assert.deepEqual(fs.readFileSync(path.join(root,'bin/blobui-mcp.exe')),fs.readFileSync(path.resolve(__dirname,'../../mcp-server/dist/blobui-mcp.exe')));
const icon=fs.readFileSync(path.join(root,'assets/icon.png'));
assert.equal(icon.subarray(0,8).toString('hex'),'89504e470d0a1a0a');
assert(icon.readUInt32BE(16)>=256);
const allowed = new Set([
  "plugin.json",
  "mcp.json",
  ".codex-plugin/plugin.json",
  ".mcp.json",
  "README.md",
  "assets/icon.png",
  "bin/blobui-mcp.exe",
  "references/workflow-rules.md",
  "skills/search-and-answer/SKILL.md",
  "skills/search-and-answer/agents/openai.yaml",
  "skills/capture-and-organize/SKILL.md",
  "skills/capture-and-organize/agents/openai.yaml",
  "skills/maintain-pages/SKILL.md",
  "skills/maintain-pages/agents/openai.yaml",
  "skills/plan-projects/SKILL.md",
  "skills/plan-projects/agents/openai.yaml",
  "skills/review-project-progress/SKILL.md",
  "skills/review-project-progress/agents/openai.yaml",
  "skills/process-meeting-notes/SKILL.md",
  "skills/process-meeting-notes/agents/openai.yaml",
  "skills/manage-types-and-templates/SKILL.md",
  "skills/manage-types-and-templates/agents/openai.yaml",
  "skills/build-queries-and-views/SKILL.md",
  "skills/build-queries-and-views/agents/openai.yaml",
  "skills/configure-kanban-calculations/SKILL.md",
  "skills/configure-kanban-calculations/agents/openai.yaml",
  "skills/configure-calendar-colors/SKILL.md",
  "skills/configure-calendar-colors/agents/openai.yaml",
  "skills/audit-organization/SKILL.md",
  "skills/audit-organization/agents/openai.yaml"
]);
assert.equal(plugin.skills, './skills/');
for (const relative of allowed) assert(fs.statSync(path.join(root, relative)).size > 0, 'Missing package file '+relative);
function inspect(dir) { for (const item of fs.readdirSync(dir, {withFileTypes:true})) { const file=path.join(dir,item.name); assert(!item.isSymbolicLink()); if(item.isDirectory()) inspect(file); else { const relative=path.relative(root,file).replaceAll('\\','/'); assert(allowed.has(relative), 'Unexpected package file '+relative); if(!relative.endsWith('.exe')&&!relative.endsWith('.png')) assert(!/[a-f0-9]{64}|Bearer\s+\S+|api_key\s*[=:]\s*['"][^'"]+/i.test(fs.readFileSync(file,'utf8')), 'Credential-like content in '+relative); } } }
inspect(root);
console.log('Anyblob metadata, paths, icon, executable and package contents passed.');
