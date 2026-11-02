const fs = require('node:fs');
const path = require('node:path');
const vm = require('node:vm');
const assert = require('node:assert/strict');
const root = path.resolve(__dirname, '../..');
const icon = fs.readFileSync(path.join(root, 'mcp-server/src/icon-data-uri.txt'), 'utf8');
const png = Buffer.from(icon.replace(/^data:image\/png;base64,/, ''), 'base64');
assert(icon.startsWith('data:image/png;base64,'));
assert.equal(png.subarray(0, 8).toString('hex'), '89504e470d0a1a0a');
assert.equal(png.readUInt32BE(16), 256);
assert.equal(png.readUInt32BE(20), 256);
const ico = fs.readFileSync(path.join(root, 'settings-app/icons/icon.ico'));
assert(ico.includes(png), 'Mention icon must match the existing app logo');
const main = fs.readFileSync(path.join(root, 'mcp-server/src/main.rs'), 'utf8');
assert(main.includes('"icons":[{"src":include_str!("icon-data-uri.txt"),"mimeType":"image/png","sizes":["256x256"]}]'), 'Initialization must advertise the embedded server icon');
const plugin = JSON.parse(fs.readFileSync(path.join(root, 'codex-plugin/src/.codex-plugin/plugin.json')));
assert.equal(plugin.version, '0.2.17');
for (const relative of ['mcp-server/Cargo.toml', 'mcp-server/Cargo.lock', 'settings-app/Cargo.lock']) {
  const source = fs.readFileSync(path.join(root, relative), 'utf8');
  assert(source.includes('name = "blobui-mcp"\nversion = "0.2.17"'), relative);
}
// Read host code only. Do not launch apps, resolve transports, or create chats.
const archive = process.argv[2];
assert(archive, 'Pass the installed app.asar to check its actual icon selector');
const fd = fs.openSync(archive, 'r');
let source;
try {
  const header = Buffer.alloc(16);
  fs.readSync(fd, header, 0, 16, 0);
  const data = Buffer.alloc(header.readUInt32LE(12));
  fs.readSync(fd, data, 0, data.length, 16);
  const base = 8 + header.readUInt32LE(4);
  function visit(files) {
    for (const [name, entry] of Object.entries(files)) {
      if (entry.files) visit(entry.files);
      else if (!entry.unpacked && /^app-initial-.*\.js$/.test(name)) {
        const bytes = Buffer.alloc(entry.size);
        fs.readSync(fd, bytes, 0, bytes.length, base + Number(entry.offset));
        if (bytes.includes('function CA(')) source = bytes.toString('utf8');
      }
    }
  }
  visit(JSON.parse(data).files);
} finally { fs.closeSync(fd); }
assert(source, 'Host icon selector changed; inspect it before updating this probe');
const context = vm.createContext({ Ve: { safeParse: data => ({ success: true, data }) } });
for (const name of ['CA', 'DJn', 'OJn', 'C1r']) {
  const start = source.indexOf(`function ${name}(`);
  const end = source.indexOf('}function ', start);
  assert(start >= 0 && end > start, `Host ${name} changed`);
  vm.runInContext(source.slice(start, end + 1), context);
}
context.wy = () => null;
assert.equal(context.C1r({}, { name: 'anyblob' }, 'anyblob'), null, 'Reproduce missing server icon');
const selected = context.C1r({}, { icons: [{ src: icon, mimeType: 'image/png', sizes: ['256x256'] }] }, 'anyblob');
assert.equal(selected.logoUrl, icon);
assert.equal(selected.logoDarkUrl, icon);
assert(source.includes('iconSmall:t.iconSmall||e.icon?.logoUrl||e.icon?.logoDarkUrl||``'), 'Host must forward server icon to inserted mentions');
console.log('PASS: logo identity, PNG dimensions, version consistency, initialization wiring, and installed host mention icon fallback');
