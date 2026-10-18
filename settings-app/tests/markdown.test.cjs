const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const workspace = path.resolve(__dirname, '../..');
const testRoot = path.join(workspace, '.test-run', `changelog-renderer-${process.pid}-${Date.now()}`);

function loadChromium() {
  for (const candidate of [process.env.PLAYWRIGHT_MODULE, 'playwright'].filter(Boolean)) {
    try { return require(candidate).chromium; } catch {}
  }
  return null;
}

const chromium = loadChromium();
const chromeCandidates = [
  process.env.CHROME_PATH,
  path.join(process.env.PROGRAMFILES || 'C:/Program Files', 'Google/Chrome/Application/chrome.exe'),
  path.join(process.env['PROGRAMFILES(X86)'] || 'C:/Program Files (x86)', 'Google/Chrome/Application/chrome.exe'),
].filter(Boolean);
const chromePath = chromeCandidates.find(candidate => fs.existsSync(candidate));

test('Markdown rendering preserves formatting and rejects active content', {
  skip: (!chromium || !chromePath) && 'Playwright and Google Chrome are required; set PLAYWRIGHT_MODULE and CHROME_PATH when they are not on the default paths',
}, async () => {
  fs.mkdirSync(testRoot, { recursive: true });
  const temp = path.join(testRoot, 'temp');
  for (const name of ['APPDATA', 'LOCALAPPDATA', 'TEMP', 'TMP', 'TMPDIR']) {
    fs.mkdirSync(path.join(temp, name), { recursive: true });
  }
  const oldEnvironment = Object.fromEntries(['APPDATA', 'LOCALAPPDATA', 'TEMP', 'TMP', 'TMPDIR'].map(name => [name, process.env[name]]));
  process.env.APPDATA = path.join(temp, 'APPDATA');
  process.env.LOCALAPPDATA = path.join(temp, 'LOCALAPPDATA');
  process.env.TEMP = path.join(temp, 'TEMP');
  process.env.TMP = process.env.TEMP;
  process.env.TMPDIR = process.env.TEMP;
  const profile = path.join(testRoot, 'chrome-profile');
  let context;
  try {
    context = await chromium.launchPersistentContext(profile, {
      executablePath: chromePath,
      headless: true,
    });
    const page = await context.newPage();
    await page.setContent('<!doctype html><main id="content" class="markdown-content"></main>');
    await page.addScriptTag({ path: path.join(workspace, 'settings-app/ui/vendor/marked.umd.js') });
    await page.addScriptTag({ path: path.join(workspace, 'settings-app/ui/markdown.js') });

    const outcome = await page.evaluate(() => {
      const png = 'data:image/png;base64,iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+/wV8AAAAASUVORK5CYII=';
      const manifest = window.BlobUI.Markdown.parseImageManifest('images:\n  - id: shot\n    src: assets/changelogs/shot.png\n    alt: Screenshot\n');
      const html = window.BlobUI.Markdown.render([
        '# Heading',
        '',
        '**bold _combined_** and ~~deleted~~',
        '',
        '[**linked _text_**](https://example.com/guide)',
        '',
        '- parent',
        '  - child',
        '',
        '- [x] complete',
        '- [ ] pending',
        '',
        '```js',
        'const mention = "@shot";',
        '```',
        '',
        '| Name | Value |',
        '| --- | --- |',
        '| Alpha | 1 |',
        '',
        '@shot',
        '',
        '<img src="@shot" alt="Raw image">',
        '<strong>Safe raw HTML</strong>',
        '<script>window.markdownXss = true</script>',
        '<img src="x" alt="blocked" onerror="window.markdownXss = true">',
        '<a href="javascript:window.markdownXss = true">blocked link</a>',
        '<iframe src="https://example.com"></iframe>',
        '<svg><script>window.markdownXss = true</script></svg>',
      ].join('\n'), { images: { shot: { ...manifest.shot, src: png } } });
      const container = document.getElementById('content');
      container.innerHTML = html;
      container.style.width = '280px';
      container.style.height = '150px';
      container.style.overflow = 'auto';
      const table = document.createElement('table');
      table.innerHTML = '<tbody><tr><td>' + 'x'.repeat(200) + '</td></tr></tbody>';
      container.append(table);
      return {
        html,
        heading: !!container.querySelector('h1'),
        nestedList: !!container.querySelector('ul ul'),
        bold: container.querySelector('strong')?.textContent,
        link: container.querySelector('a[href="https://example.com/guide"]')?.innerHTML,
        code: container.querySelector('pre code')?.textContent,
        imageCount: container.querySelectorAll('img').length,
        dataImages: [...container.querySelectorAll('img')].every(image => image.src.startsWith('data:image/png;base64,')),
        taskCheckboxes: [...container.querySelectorAll('input[type="checkbox"]')].map(input => ({ disabled: input.disabled, checked: input.checked })),
        activeTags: [...container.querySelectorAll('script, iframe, svg')].length,
        eventAttributes: [...container.querySelectorAll('*')].some(element => [...element.attributes].some(attribute => attribute.name.startsWith('on'))),
        unsafeLinks: [...container.querySelectorAll('a')].some(anchor => /^(?:javascript|file):/i.test(anchor.getAttribute('href') || '')),
        overflow: container.scrollWidth > container.clientWidth,
      };
    });

    assert(outcome.heading, 'Markdown headings should be preserved.');
    assert(outcome.nestedList, 'Nested lists should be preserved.');
    assert.match(outcome.bold, /bold/);
    assert.equal(outcome.link, '<strong>linked <em>text</em></strong>', 'Links should preserve formatted Markdown text.');
    assert.match(outcome.code, /@shot/, 'Image mentions in fenced code should remain text.');
    assert.equal(outcome.imageCount, 2, 'Markdown and raw HTML aliases should both resolve to images.');
    assert(outcome.dataImages, 'Rendered images should use the supplied local image bytes.');
    assert.deepEqual(outcome.taskCheckboxes, [{ disabled: true, checked: true }, { disabled: true, checked: false }]);
    assert.equal(outcome.activeTags, 0, 'Scripts, iframes, and SVG must be removed.');
    assert.equal(outcome.eventAttributes, false, 'Event-handler attributes must be removed.');
    assert.equal(outcome.unsafeLinks, false, 'Script and file links must be removed.');
    assert.equal(outcome.overflow, true, 'Wide tables should remain horizontally scrollable.');

    const invalidManifests = [
      'images:\n  - id: escape\n    src: assets/changelogs/../outside.png\n',
      'images:\n  - id: vector\n    src: assets/changelogs/vector.svg\n',
    ];
    for (const manifest of invalidManifests) {
      await assert.rejects(page.evaluate(source => window.BlobUI.Markdown.parseImageManifest(source), manifest));
    }
  } finally {
    if (context) await context.close();
    for (const [name, value] of Object.entries(oldEnvironment)) {
      if (value === undefined) delete process.env[name];
      else process.env[name] = value;
    }
    fs.rmSync(testRoot, { recursive: true, force: true });
  }
});
