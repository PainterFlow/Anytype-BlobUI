const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');

const workspace = path.resolve(__dirname, '../..');
const source = fs.readFileSync(path.join(workspace, 'settings-app/ui/docs-data.js'), 'utf8');
const context = { fetch: global.fetch };
vm.runInNewContext(source, context, { filename: 'docs-data.js' });
const DocsData = context.BlobUI.DocsData;

function article(id, category, name, extra = '', body = 'A short article.') {
  return { id, source: `---\ncategory: ${category}\n${extra}icon: Aa\nname: ${name}\n---\n\n${body}` };
}

test('catalog sorts articles alphabetically and resolves a parent by slug or unique name', () => {
  const catalog = DocsData.buildCatalog([
    article('typography', 'getting-started', 'Typography', 'parent: Customize appearance\n'),
    article('customize-appearance', 'getting-started', 'Customize appearance'),
    article('accent-colors', 'getting-started', 'Accent colors', 'parent: customize-appearance\n'),
    article('api-key', 'reference', 'Connect an Anytype API key'),
  ]);

  assert.deepEqual(Array.from(catalog.categories, ({ id }) => id), ['getting-started', 'reference']);
  assert.deepEqual(Array.from(catalog.articles, ({ id }) => id), ['accent-colors', 'api-key', 'customize-appearance', 'typography']);
  assert.equal(catalog.articles.find(({ id }) => id === 'typography').parent, 'customize-appearance');
  assert.equal(catalog.articles.find(({ id }) => id === 'accent-colors').parent, 'customize-appearance');
});

test('article parsing returns frontmatter and Markdown body separately', () => {
  const parsed = DocsData.parseArticle(article('sample', 'reference', 'Sample', '', '# Heading\n\nText.').source, 'sample.md');
  assert.equal(parsed.id, 'sample');
  assert.equal(parsed.category, 'reference');
  assert.equal(parsed.parent, null);
  assert.equal(parsed.markdown, '# Heading\n\nText.');
});

test('catalog rejects unknown metadata references, duplicates, and invalid parent trees', () => {
  assert.throws(() => DocsData.buildCatalog([article('missing-category', 'other', 'Missing')]), /unknown category/);
  assert.throws(() => DocsData.buildCatalog([article('child', 'getting-started', 'Child', 'parent: absent\n')]), /unknown parent/);
  assert.throws(() => DocsData.buildCatalog([article('same', 'reference', 'First'), article('same', 'reference', 'Second')]), /duplicate article id/);
  assert.throws(() => DocsData.buildCatalog([
    article('parent', 'getting-started', 'Parent', 'parent: child\n'),
    article('child', 'getting-started', 'Child', 'parent: parent\n'),
  ]), /cycle|one parent level/);
  assert.throws(() => DocsData.buildCatalog([
    article('parent', 'getting-started', 'Parent'),
    article('child', 'reference', 'Child', 'parent: parent\n'),
  ]), /different category/);
});

test('catalog rejects self-parenting and acyclic trees deeper than one parent level', () => {
  assert.throws(() => DocsData.buildCatalog([
    article('self', 'getting-started', 'Self', 'parent: self\n'),
  ]), /cycle/);
  assert.throws(() => DocsData.buildCatalog([
    article('root', 'getting-started', 'Root'),
    article('child', 'getting-started', 'Child', 'parent: root\n'),
    article('grandchild', 'getting-started', 'Grandchild', 'parent: child\n'),
  ]), /one parent level/);
});

test('catalog rejects ambiguous parent names and unknown prototype aliases', () => {
  assert.throws(() => DocsData.buildCatalog([
    article('first-parent', 'getting-started', 'Shared name'),
    article('second-parent', 'getting-started', 'Shared name'),
    article('child', 'getting-started', 'Child', 'parent: Shared name\n'),
  ]), /ambiguous.*use its slug/);
  assert.equal(DocsData.resolveImage('__proto__'), '');
});

test('frontmatter rejects duplicate keys and article body is required', () => {
  assert.throws(() => DocsData.parseArticle('---\ncategory: getting-started\ncategory: reference\nicon: x\nname: Duplicate\n---\nbody', 'bad'), /repeats "category"/);
  assert.throws(() => DocsData.parseArticle('---\ncategory: getting-started\nicon: x\nname: Empty\n---\n', 'empty'), /body must not be empty/);
});

test('image aliases only return allowlisted local assets', () => {
  assert.equal(DocsData.resolveImage('blobui-mark'), './cdn/blobui-mark.svg');
  assert.equal(DocsData.resolveImage('../secret'), '');
  assert.equal(DocsData.resolveImage('https://example.com/image.svg'), '');
});
