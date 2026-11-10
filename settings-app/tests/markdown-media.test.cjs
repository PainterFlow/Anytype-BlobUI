const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const workspace = path.resolve(__dirname, '../..');
const uiRoot = path.join(workspace, 'settings-app/ui');
const source = fs.readFileSync(path.join(uiRoot, 'markdown-media.js'), 'utf8');
const context = {};
vm.runInNewContext(source, context, { filename: 'markdown-media.js' });
const MarkdownMedia = context.BlobUI.MarkdownMedia;

test('Markdown media serves only registered local image and video assets', () => {
  assert.equal(MarkdownMedia.resolveImage('calendar-event-colors'), './cdn/calendar-event-colors.png');
  assert.equal(MarkdownMedia.resolveImage('blobui-mark'), './cdn/blobui-mark.svg');
  assert.equal(MarkdownMedia.resolveImage('__proto__'), '');
  assert.equal(MarkdownMedia.resolveImage('../secret'), '');
  assert.equal(MarkdownMedia.resolveImage('https://example.com/image.svg'), '');
  assert.equal(MarkdownMedia.videoAliases['kanban-compact-collapsing'], './cdn/kanban-compact-collapsing.mp4');
  assert.equal(MarkdownMedia.videoAliases['__proto__'], undefined);

  for (const assetPath of [...Object.values(MarkdownMedia.imageAliases), ...Object.values(MarkdownMedia.videoAliases)]) {
    assert.match(assetPath, /^\.\/cdn\/[A-Za-z0-9_-]+\.(?:svg|png|mp4)$/);
    assert.equal(fs.existsSync(path.join(uiRoot, assetPath)), true, `${assetPath} exists`);
  }
});
