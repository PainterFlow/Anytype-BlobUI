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

test('Markdown image aliases come from md-images.yaml and resolve only to bundled local files', async () => {
  assert.equal(imageManifest['calendar-event-colors'].src, './cdn/calendar-event-colors.png');
  assert.equal(imageManifest['blobui-mark'].src, './cdn/blobui-mark.svg');
  assert.equal(imageManifest.__proto__, undefined);
  assert.throws(() => Markdown.parseImageManifest('images:\n  - id: escape\n    src: ./cdn/../secret.png\n'));

  for (const image of Object.values(imageManifest)) {
    if (!image.src.startsWith('./cdn/')) continue;
    assert.match(image.src, /^\.\/cdn\/[A-Za-z0-9_-]+\.(?:svg|png|jpe?g|gif|webp)$/i);
    assert.equal(fs.existsSync(path.join(uiRoot, image.src)), true, `${image.src} exists`);
  }

  assert.equal((await MarkdownMedia.loadImage('../secret.png')), null);
  const loaded = await MarkdownMedia.loadImage('./cdn/calendar-event-colors.png');
  assert.equal(loaded.src, 'data:image/png;base64,AQIDBA==');
  assert.equal(MarkdownMedia.videoAliases['kanban-compact-collapsing'], './cdn/kanban-compact-collapsing.mp4');
  assert.equal(MarkdownMedia.videoAliases.__proto__, undefined);
  for (const source of Object.values(MarkdownMedia.videoAliases)) {
    assert.match(source, /^\.\/cdn\/[A-Za-z0-9_-]+\.mp4$/);
    assert.equal(fs.existsSync(path.join(uiRoot, source)), true, `${source} exists`);
  }
});
