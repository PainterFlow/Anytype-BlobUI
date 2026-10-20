const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const workspace = path.resolve(__dirname, '../..');

function loadChromium() {
  const candidates = [process.env.PLAYWRIGHT_MODULE, 'playwright'].filter(Boolean);
  for (const candidate of candidates) {
    try {
      return require(candidate).chromium;
    } catch {}
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

test('documentation navigation selects articles, expands one child level, and sanitizes Markdown', {
  skip: (!chromium || !chromePath) && 'Playwright and Google Chrome are required; set PLAYWRIGHT_MODULE and CHROME_PATH when they are not on the default paths',
}, async () => {
  const browser = await chromium.launch({ executablePath: chromePath, headless: true });
  try {
    const page = await browser.newPage();
    await page.setContent(`<!doctype html><html><body>
      <nav id="documentation-navigation" aria-label="Documentation articles"></nav>
      <main id="documentation-content"><div id="documentation-article"></div>
        <div id="documentation-load-error" hidden><button id="documentation-retry">Try again</button></div>
      </main>
    </body></html>`);
    await page.addScriptTag({ path: path.join(workspace, 'settings-app/ui/vendor/marked.umd.js') });
    await page.addScriptTag({ path: path.join(workspace, 'settings-app/ui/markdown.js') });
    await page.addScriptTag({ path: path.join(workspace, 'settings-app/ui/disclosure.js') });
    await page.evaluate(() => {
      window.BlobUI.DocsData = {
        async load() {
          return {
            categories: [
              { id: 'getting-started', name: 'Getting Started' },
              { id: 'reference', name: 'Reference' },
            ],
            articles: [
              { id: 'parent', category: 'getting-started', parent: null, icon: '', name: 'Zebra guide', markdown: '# Parent article\n\n<script>window.docsXss = true</script>' },
              { id: 'child-b', category: 'getting-started', parent: 'parent', icon: '', name: 'Child B', markdown: '# Child article' },
              { id: 'child-a', category: 'getting-started', parent: 'parent', icon: '', name: 'Child A', markdown: '# Child article' },
              { id: 'first', category: 'getting-started', parent: null, icon: '', name: 'Alpha guide', markdown: '# First article' },
              { id: 'reference', category: 'reference', parent: null, icon: '', name: 'API reference', markdown: '# Reference article' },
            ],
          };
        },
        resolveImage() { return ''; },
      };
    });
    await page.addScriptTag({ path: path.join(workspace, 'settings-app/ui/docs.js') });
    await page.evaluate(() => window.BlobUI.DocsUI.load());

    assert.equal(await page.locator('#documentation-article h1').textContent(), 'First article', 'The first code-ordered category and alphabetical root should open by default.');
    assert.equal(await page.locator('[data-article-id="first"]').getAttribute('aria-current'), 'page');
    assert.equal(await page.evaluate(() => window.docsXss), undefined, 'Unsafe Markdown scripts must be removed.');

    const parent = page.locator('[data-article-id="parent"]');
    await parent.click();
    assert.equal(await parent.getAttribute('aria-expanded'), 'true', 'Selecting a parent article should also expand it.');
    assert.equal(await parent.getAttribute('aria-current'), 'page');
    const childIds = await page.locator('#documentation-children-parent [data-article-id]').evaluateAll(nodes => nodes.map(node => node.dataset.articleId));
    assert.deepEqual(childIds, ['child-a', 'child-b'], 'Children should be alphabetized.');

    await page.locator('[data-article-id="child-a"]').click();
    assert.equal(await page.locator('#documentation-article h1').textContent(), 'Child article');
    assert.equal(await page.locator('[data-article-id="child-a"]').getAttribute('aria-current'), 'page');
    await parent.click();
    assert.equal(await parent.getAttribute('aria-expanded'), 'false', 'Selecting the expanded parent should collapse its children.');
    assert.equal(await parent.getAttribute('aria-current'), 'page');
  } finally {
    await browser.close();
  }
});

test('disclosure controller supports reusable panels, rapid reversal, focus safety, and reduced motion', {
  skip: (!chromium || !chromePath) && 'Playwright and Google Chrome are required; set PLAYWRIGHT_MODULE and CHROME_PATH when they are not on the default paths',
}, async () => {
  const browser = await chromium.launch({ executablePath: chromePath, headless: true });
  try {
    const page = await browser.newPage();
    await page.setContent(`<!doctype html><html><body>
      <section id="settings-group">
        <button id="settings-toggle" type="button" data-disclosure-toggle aria-controls="settings-panel" aria-expanded="false">Advanced settings</button>
        <div id="settings-panel" data-disclosure-panel data-disclosure-state="closed" hidden inert>
          <div class="documentation-children-inner" data-disclosure-content>
            <div style="height: 100px"><button id="settings-control" type="button">Setting</button></div>
          </div>
        </div>
      </section>
    </body></html>`);
    await page.addStyleTag({ path: path.join(workspace, 'settings-app/ui/styles.css') });
    await page.addScriptTag({ path: path.join(workspace, 'settings-app/ui/disclosure.js') });
    await page.evaluate(() => {
      window.disclosureController = window.BlobUI.Disclosure.attach(document.getElementById('settings-group'));
      window.originalSettingsPanel = document.getElementById('settings-panel');
    });

    const openingHeights = await page.evaluate(async () => {
      const panel = document.getElementById('settings-panel');
      window.disclosureController.setExpanded(document.getElementById('settings-toggle'), true);
      let transition = null;
      const startedAt = performance.now();
      while (!transition && performance.now() - startedAt < 500) {
        await new Promise(requestAnimationFrame);
        transition = panel.getAnimations().find(animation => animation.transitionProperty === 'grid-template-rows' && animation.playState === 'running');
      }
      if (!transition) return { mid: -1, full: -1 };
      while (Number(transition.currentTime) < 70) await new Promise(requestAnimationFrame);
      const mid = panel.getBoundingClientRect().height;
      await transition.finished;
      return { mid, full: panel.getBoundingClientRect().height };
    });
    const midOpenHeight = openingHeights.mid;
    assert.ok(midOpenHeight > 0 && midOpenHeight < 100, `The real panel transition should be partway open at 70ms, got ${midOpenHeight}px.`);
    const fullOpenHeight = openingHeights.full;
    assert.ok(Math.abs(fullOpenHeight - 100) < 1, `The panel should reach its 100px content height, got ${fullOpenHeight}px.`);

    const closeStarted = await page.evaluate(async () => {
      const panel = document.getElementById('settings-panel');
      window.disclosureController.setExpanded(document.getElementById('settings-toggle'), false);
      let transition = null;
      const startedAt = performance.now();
      while (!transition && performance.now() - startedAt < 500) {
        await new Promise(requestAnimationFrame);
        transition = panel.getAnimations().find(animation => animation.transitionProperty === 'grid-template-rows' && animation.playState === 'running');
      }
      if (!transition) return false;
      while (Number(transition.currentTime) < 70) await new Promise(requestAnimationFrame);
      return true;
    });
    assert.equal(closeStarted, true, 'The panel should be in a real closing transition before reversal.');
    await page.evaluate(() => window.disclosureController.setExpanded(document.getElementById('settings-toggle'), true));
    await page.waitForTimeout(300);

    assert.equal(await page.locator('#settings-toggle').getAttribute('aria-expanded'), 'true');
    assert.equal(await page.locator('#settings-panel').evaluate(panel => panel.hidden), false, 'Reversing an in-progress close must not hide a panel after its transition fallback fires.');
    assert.equal(await page.locator('#settings-panel').getAttribute('data-disclosure-state'), 'open');
    assert.equal(await page.locator('#settings-panel').evaluate(panel => panel.inert), false);
    assert.equal(await page.locator('#settings-panel').evaluate(panel => panel === window.originalSettingsPanel), true, 'The controller must preserve the panel node through toggles.');

    await page.emulateMedia({ reducedMotion: 'reduce' });
    await page.locator('#settings-control').focus();
    await page.evaluate(() => window.disclosureController.setExpanded(document.getElementById('settings-toggle'), false));
    assert.equal(await page.locator('#settings-toggle').getAttribute('aria-expanded'), 'false');
    assert.equal(await page.locator('#settings-panel').evaluate(panel => panel.hidden), true, 'Reduced motion should collapse immediately.');
    assert.equal(await page.locator('#settings-panel').evaluate(panel => panel.inert), true);
    assert.equal(await page.evaluate(() => document.activeElement.id), 'settings-toggle', 'Focus should move out of a panel before it is hidden.');

    await page.evaluate(() => window.disclosureController.setExpanded(document.getElementById('settings-toggle'), true));
    assert.equal(await page.locator('#settings-panel').getAttribute('data-disclosure-state'), 'open', 'Reduced motion should also open without a deferred animation frame.');
  } finally {
    await browser.close();
  }
});

test('bundled documentation loads over HTTP with real Markdown, icons, and image aliases', {
  skip: (!chromium || !chromePath) && 'Playwright and Google Chrome are required; set PLAYWRIGHT_MODULE and CHROME_PATH when they are not on the default paths',
}, async () => {
  const uiRoot = path.join(workspace, 'settings-app/ui');
  const testRoot = path.join(workspace, '.test-run', 'docs-ui-integration');
  const profile = path.join(testRoot, 'chrome-profile');
  const screenshot = path.join(workspace, '.test-run', 'docs-preview.png');
  fs.mkdirSync(testRoot, { recursive: true });
  fs.mkdirSync(path.dirname(screenshot), { recursive: true });

  const mimeTypes = {
    '.css': 'text/css; charset=utf-8',
    '.html': 'text/html; charset=utf-8',
    '.js': 'text/javascript; charset=utf-8',
    '.md': 'text/markdown; charset=utf-8',
    '.png': 'image/png',
    '.svg': 'image/svg+xml',
  };
  const hostHtml = `<!doctype html><html lang="en" data-theme="dark"><head>
    <meta charset="utf-8"><meta name="viewport" content="width=device-width, initial-scale=1">
    <link rel="stylesheet" href="/styles.css"><title>Documentation preview</title></head><body>
    <header class="window-titlebar"><span class="window-title">BlobUI Settings</span><div class="window-controls">
      <button id="documentation-back" class="window-control" type="button" aria-label="Back to settings" title="Back to settings"><svg viewBox="0 0 24 24" aria-hidden="true"><path d="m9 4-6 6 6 6M3 10h11a7 7 0 0 1 7 7v3"/></svg></button>
    </div></header>
    <section id="documentation" class="documentation" aria-label="Documentation">
      <div class="documentation-layout">
        <nav id="documentation-navigation" class="documentation-navigation" aria-label="Documentation articles"></nav>
        <main id="documentation-content" class="documentation-content" aria-label="Article" tabindex="-1">
          <div id="documentation-article" class="documentation-article markdown-content"></div>
          <div id="documentation-load-error" class="documentation-load-error" role="alert" hidden>
            <p>Documentation could not be loaded.</p><button id="documentation-retry" class="banner-image-button" type="button">Try again</button>
          </div>
        </main>
      </div>
    </section>
    <script src="/vendor/marked.umd.js"></script><script src="/markdown.js"></script>
    <script src="/docs-data.js"></script><script src="/disclosure.js"></script><script src="/docs.js"></script>
    <script>window.docsReady = window.BlobUI.DocsUI.load();</script>
  </body></html>`;

  let context;
  try {
    context = await chromium.launchPersistentContext(profile, {
      executablePath: chromePath,
      headless: true,
      viewport: { width: 960, height: 740 },
    });
    await context.route('http://docs.test/**', async route => {
      const requestUrl = new URL(route.request().url());
      if (requestUrl.pathname === '/__docs-host') {
        await route.fulfill({ status: 200, contentType: mimeTypes['.html'], body: hostHtml });
        return;
      }
      const relativePath = decodeURIComponent(requestUrl.pathname).replace(/^\/+/, '');
      const filePath = path.resolve(uiRoot, relativePath);
      const relative = path.relative(uiRoot, filePath);
      if (!relative || relative === '..' || relative.startsWith(`..${path.sep}`) || path.isAbsolute(relative)) {
        await route.fulfill({ status: 404, body: 'Not found' });
        return;
      }
      try {
        const content = fs.readFileSync(filePath);
        await route.fulfill({
          status: 200,
          contentType: mimeTypes[path.extname(filePath).toLowerCase()] || 'application/octet-stream',
          body: content,
        });
      } catch {
        await route.fulfill({ status: 404, body: 'Not found' });
      }
    });
    const page = await context.newPage();
    await page.goto('http://docs.test/__docs-host');
    await page.evaluate(() => window.docsReady);

    const selectedParent = page.locator('[data-article-id="customize-appearance"]');
    assert.equal(await selectedParent.getAttribute('aria-current'), 'page', 'The first root article in the first category should open by default.');
    assert.equal(await selectedParent.getAttribute('aria-expanded'), 'true', 'The selected parent should start expanded.');
    const initialChildren = await page.locator('#documentation-children-customize-appearance [data-article-id]')
      .evaluateAll(nodes => nodes.map(node => node.dataset.articleId));
    assert.deepEqual(initialChildren, ['accent-colors', 'typography'], 'Bundled child articles should be alphabetized and visible.');

    const svgIcon = page.locator('#documentation-navigation [data-article-id="customize-appearance"] img');
    await page.waitForFunction(() => {
      const image = document.querySelector('#documentation-navigation [data-article-id="customize-appearance"] img');
      return image && image.complete && image.naturalWidth > 0;
    });
    assert.match(await svgIcon.getAttribute('src'), /blobui-mark\.svg$/);

    await page.locator('[data-article-id="accent-colors"]').click();
    const swatches = page.locator('#documentation-article img[alt="Theme color swatches"]');
    await page.waitForFunction(() => {
      const image = document.querySelector('#documentation-article img[alt="Theme color swatches"]');
      return image && image.complete && image.naturalWidth > 0;
    });
    assert.match(await swatches.getAttribute('src'), /^data:image\/png;base64,/);
    assert.match(await page.locator('#documentation-article').textContent(), /tests image aliases and fenced code rendering/);
    await page.screenshot({ path: screenshot });
  } finally {
    if (context) await context.close();
    fs.rmSync(profile, { recursive: true, force: true });
  }
});
