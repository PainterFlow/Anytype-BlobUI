const fs = require('node:fs');
const path = require('node:path');
const assert = require('node:assert/strict');
const root = path.resolve(process.argv[2] || 'codex-plugin/src');
const skills = fs.readdirSync(path.join(root, 'skills')).sort();
assert.equal(skills.length, 11);
for (const name of skills) {
  assert.match(name, /^[a-z0-9-]{1,63}$/);
  const folder = path.join(root, 'skills', name);
  const text = fs.readFileSync(path.join(folder, 'SKILL.md'), 'utf8');
  assert(text.startsWith(`---\nname: ${name}\ndescription: `));
  assert.match(text, /\n---\n\n/);
  assert(!/TODO|TBD|\[insert|\[placeholder/i.test(text));
  for (const [, target] of text.matchAll(/\]\(([^)]+)\)/g)) {
    if (/^[a-z]+:/.test(target)) continue;
    const resolved = path.resolve(folder, target);
    assert(resolved.startsWith(root + path.sep), 'Reference escapes package');
    assert(fs.statSync(resolved).isFile(), 'Missing skill reference');
  }
  const metadata = fs.readFileSync(path.join(folder, 'agents/openai.yaml'), 'utf8');
  assert(metadata.includes('value: "anyblob"'));
  assert(metadata.includes('$' + name));
  assert(!metadata.includes('allow_implicit_invocation: false'));
}
console.log('Eleven skill entrypoints, metadata and packaged references passed.');
