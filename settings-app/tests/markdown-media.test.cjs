const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const workspace = path.resolve(__dirname, '../..');
const uiRoot = path.join(workspace, 'settings-app/ui');
const manifestText = fs.readFileSync(path.join(workspace, 'md-images.yaml'), 'utf8');
const context = {
  fetch: async () => ({
    ok: true,
    headers: { get: name => name.toLowerCase() === 'content-type' ? 'image/png' : '4' },
    arrayBuffer: async () => Uint8Array.from([1, 2, 3, 4]).buffer,
  }),
  btoa: value => Buffer.from(value, 'binary').toString('base64'),
};
vm.runInNewContext(fs.readFileSync(path.join(uiRoot, 'markdown.js'), 'utf8'), context, { filename: 'markdown.js' });
vm.runInNewContext(fs.readFileSync(path.join(uiRoot, 'markdown-media.js'), 'utf8'), context, { filename: 'markdown-media.js' });
const Markdown = context.BlobUI.Markdown;
const MarkdownMedia = context.BlobUI.MarkdownMedia;
const imageManifest = Markdown.parseMediaManifest(manifestText).images;

test('Markdown keeps local image loading and video aliases available with no registered image aliases', async () => {
  assert.deepEqual(Object.keys(imageManifest), [], 'The current Markdown has no registered image aliases.');
  assert.equal(imageManifest.__proto__, undefined);
  assert.throws(() => Markdown.parseImageManifest('images:\n  - id: escape\n    src: ./cdn/../secret.png\n'));

  assert.equal((await MarkdownMedia.loadImage('../secret.png')), null);
  const loaded = await MarkdownMedia.loadImage('./cdn/test-fixture.png');
  assert.equal(loaded.src, 'data:image/png;base64,AQIDBA==');
  assert.equal(MarkdownMedia.videoAliases['kanban-compact-collapsing'], './cdn/kanban-compact-collapsing.mp4');
  assert.equal(MarkdownMedia.videoAliases.__proto__, undefined);
  for (const source of Object.values(MarkdownMedia.videoAliases)) {
    assert.match(source, /^\.\/cdn\/[A-Za-z0-9_-]+\.mp4$/);
    assert.equal(fs.existsSync(path.join(uiRoot, source)), true, `${source} exists`);
  }
});
