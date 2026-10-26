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

test('production catalog loads all eleven complete root articles and README links resolve', async () => {
  const docsRoot = path.join(workspace, 'settings-app/ui/docs');
  const expected = ['api-and-calendar-colors', 'features', 'installation-and-removal', 'introduction', 'kanban-layouts-and-formulas', 'settings-ui-refresh', 'setup-and-architecture', 'testing-and-releases', 'text-effects-and-previews', 'theme-and-documentation-development', 'updating'];
  const requested = [];
  context.fetch = async url => {
    requested.push(url);
    return { ok: true, text: async () => fs.readFileSync(path.join(workspace, 'settings-app/ui', url), 'utf8') };
  };
  const catalog = await DocsData.load();
  assert.deepEqual(Array.from(catalog.categories, item => [item.id, item.name]), [['getting-started', 'User guide'], ['development', 'Development']]);
  assert.deepEqual(Array.from(catalog.articles, item => item.id).sort(), expected);
  assert.deepEqual(fs.readdirSync(docsRoot).filter(name => name.endsWith('.md')).sort(), expected.map(id => id + '.md'));
  assert.equal(requested.length, 11);
  for (const category of catalog.categories) {
    const group = catalog.articles.filter(article => article.category === category.id);
    assert.deepEqual(Array.from(group, article => article.index), Array.from(group, (_, index) => index + 1));
  }
  for (const article of catalog.articles) {
    assert.equal(article.parent, null);
    assert.ok(article.markdown.length > 700);
    assert.doesNotMatch(article.markdown, /documentation test page|child page tests|separate-category test page|example\.com/i);
    for (const match of article.markdown.matchAll(/https:\/\/github\.com\/PainterFlow\/Anytype-BlobUI\/blob\/dev\/([^\s)]+)/g)) {
      assert.ok(fs.existsSync(path.join(workspace, match[1])), match[1]);
    }
  }
  const readme = fs.readFileSync(path.join(workspace, 'README.md'), 'utf8');
  for (const id of expected) assert.ok(readme.includes(`](settings-app/ui/docs/${id}.md)`));
  for (const [, target] of readme.matchAll(/!?\[[^\]]*\]\(([^)]+)\)/g)) {
    if (!/^https?:/.test(target)) assert.ok(fs.existsSync(path.join(workspace, target)), target);
  }
});

function article(id, category, name, extra = '', body = 'A short article.') {
  return { id, source: `---\ncategory: ${category}\n${extra}icon: Aa\nname: ${name}\n---\n\n${body}` };
}

test('indices are positive integers, restart per category, and override alphabetical order', () => {
  const catalog = DocsData.buildCatalog([
    article('later', 'getting-started', 'Alpha', 'index: 2\n'),
    article('first', 'getting-started', 'Zebra', 'index: 1\n'),
    article('dev-first', 'development', 'Dev', 'index: 1\n'),
    article('unindexed', 'getting-started', 'Aardvark'),
  ]);
  assert.deepEqual(Array.from(catalog.articles.filter(item => item.category === 'getting-started'), item => item.id), ['first', 'later', 'unindexed']);
  for (const value of ['0', '-1', '1.5', 'abc', '9007199254740992']) {
    assert.throws(() => DocsData.parseArticle(article('invalid', 'development', 'Invalid', `index: ${value}\n`).source, 'invalid'), /index.*positive integer/);
  }
});

test('catalog sorts articles alphabetically and resolves a parent by slug or unique name', () => {
  const catalog = DocsData.buildCatalog([
    article('typography', 'getting-started', 'Typography', 'parent: Customize appearance\n'),
    article('customize-appearance', 'getting-started', 'Customize appearance'),
    article('accent-colors', 'getting-started', 'Accent colors', 'parent: customize-appearance\n'),
    article('api-key', 'development', 'Connect an Anytype API key'),
  ]);

  assert.deepEqual(Array.from(catalog.categories, ({ id }) => id), ['getting-started', 'development']);
  assert.deepEqual(Array.from(catalog.articles, ({ id }) => id), ['accent-colors', 'api-key', 'customize-appearance', 'typography']);
  assert.equal(catalog.articles.find(({ id }) => id === 'typography').parent, 'customize-appearance');
  assert.equal(catalog.articles.find(({ id }) => id === 'accent-colors').parent, 'customize-appearance');
});

test('article parsing returns frontmatter and Markdown body separately', () => {
  const parsed = DocsData.parseArticle(article('sample', 'development', 'Sample', '', '# Heading\n\nText.').source, 'sample.md');
  assert.equal(parsed.id, 'sample');
  assert.equal(parsed.category, 'development');
  assert.equal(parsed.parent, null);
  assert.equal(parsed.markdown, '# Heading\n\nText.');
});

test('catalog rejects unknown metadata references, duplicates, and invalid parent trees', () => {
  assert.throws(() => DocsData.buildCatalog([article('missing-category', 'other', 'Missing')]), /unknown category/);
  assert.throws(() => DocsData.buildCatalog([article('child', 'getting-started', 'Child', 'parent: absent\n')]), /unknown parent/);
  assert.throws(() => DocsData.buildCatalog([article('same', 'development', 'First'), article('same', 'development', 'Second')]), /duplicate article id/);
  assert.throws(() => DocsData.buildCatalog([
    article('parent', 'getting-started', 'Parent', 'parent: child\n'),
    article('child', 'getting-started', 'Child', 'parent: parent\n'),
  ]), /cycle|one parent level/);
  assert.throws(() => DocsData.buildCatalog([
    article('parent', 'getting-started', 'Parent'),
    article('child', 'development', 'Child', 'parent: parent\n'),
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
