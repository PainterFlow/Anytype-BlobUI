const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');

const workspace = path.resolve(__dirname, '../..');
const testRoot = path.join(workspace, '.test-run', 'hover-preview-dom');
const temp = path.join(testRoot, 'temp');

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

function compileConditions(source, settings) {
  return source.replace(
    /\/\*\s*@blobui-if(-not)?\s+([A-Za-z][A-Za-z0-9]*)\s*\*\/([\s\S]*?)\/\*\s*@blobui-endif(-not)?\s+([A-Za-z][A-Za-z0-9]*)\s*\*\//g,
    (_match, negation, name, content, endNegation, endName) => {
      assert.equal(name, endName, `Mismatched CSS condition: ${name} / ${endName}`);
      assert.equal(negation, endNegation, `Mismatched CSS condition polarity for ${name}`);
      const enabled = settings[name] === true;
      return enabled !== (negation === '-not') ? content : '';
    }
  );
}

test('hover preview toggles hide only their own cards and preserve clicks', {
  skip: (!chromium || !chromePath) && 'Playwright and Google Chrome are required; set PLAYWRIGHT_MODULE and CHROME_PATH when they are not on the default paths',
}, async () => {
  const relativeTestRoot = path.relative(workspace, testRoot);
  assert.ok(relativeTestRoot && !relativeTestRoot.startsWith(`..${path.sep}`) && !path.isAbsolute(relativeTestRoot), 'Browser test artifacts must stay inside the workspace');
  fs.mkdirSync(temp, { recursive: true });
  process.env.TEMP = temp;
  process.env.TMP = temp;
  process.env.TMPDIR = temp;
  const cssSources = ['60-accessibility.css', '60-tab-hover-preview.css']
    .map(file => fs.readFileSync(path.join(workspace, 'Dev', file), 'utf8'))
    .join('\n');
  const tabOffCss = compileConditions(cssSources, {
    reducedMotion: false,
    tabHoverEffect: false,
    linkHoverEffect: true,
    objectMentionHoverEffect: true,
  });
  const profile = path.join(testRoot, 'chrome-profile');
  let context;
  try {
    context = await chromium.launchPersistentContext(profile, {
      executablePath: chromePath,
      headless: true,
    });
    const page = await context.newPage();
    for (const linkHoverEffect of [false, true]) {
      for (const objectMentionHoverEffect of [false, true]) {
        const css = compileConditions(cssSources, {
          reducedMotion: false,
          tabHoverEffect: true,
          linkHoverEffect,
          objectMentionHoverEffect,
        });
        await page.setContent(`<!doctype html>
          <style>
            body { margin: 0; }
            .previewWrapper { display: block; position: fixed; left: 700px; top: 500px; }
          </style>
          <style id="blobui-css">${css}</style>
          <div class="pageMain"><div class="editor"><div class="blockText"><div class="editable">
            <a id="link-source" href="https://example.test">Link source</a>
            <a id="mention-source" href="anytype://object"><markupmention><span>Object source</span></markupmention></a>
            <a id="blob-source" href="blob://text.sfx/neon">Blob source</a>
          </div></div></div></div>
          <div id="preview" class="previewWrapper"></div>`);
        await page.evaluate(() => {
          window.clicked = [];
          for (const id of ['link-source', 'mention-source']) {
            document.getElementById(id).addEventListener('click', event => {
              event.preventDefault();
              window.clicked.push(id);
            });
          }
        });

        const checkPreview = async (markup, expectedVisible, label) => {
          await page.locator('#preview').evaluate((element, html) => { element.innerHTML = html; }, markup);
          const display = await page.locator('#preview').evaluate(element => getComputedStyle(element).display);
          assert.equal(display !== 'none', expectedVisible, `${label}: got display ${display}`);
        };
        const checkSourceClick = async id => {
          const pointerEvents = await page.locator(`#${id}`).evaluate(element => getComputedStyle(element).pointerEvents);
          assert.notEqual(pointerEvents, 'none', `${id} lost pointer interaction`);
          await page.locator(`#${id}`).click();
          assert.ok(await page.evaluate(sourceId => window.clicked.includes(sourceId), id), `${id} stopped receiving clicks`);
        };

        await checkPreview('<div class="previewLink">Link preview</div>', linkHoverEffect, `link=${linkHoverEffect}, mention=${objectMentionHoverEffect} link card`);
        await checkSourceClick('link-source');
        await checkPreview('<div class="previewDefault"><div class="previewHeader">Mention preview</div></div>', objectMentionHoverEffect, `link=${linkHoverEffect}, mention=${objectMentionHoverEffect} default mention card`);
        await checkSourceClick('mention-source');
        await checkPreview('<div class="previewObject">Object preview</div>', objectMentionHoverEffect, `link=${linkHoverEffect}, mention=${objectMentionHoverEffect} object card`);
        await checkSourceClick('mention-source');
        await checkPreview('<div class="previewTab"><div class="previewObject">Tab preview</div></div>', true, `link=${linkHoverEffect}, mention=${objectMentionHoverEffect} tab card`);
      }
    }

    const tabOffCssTag = `<style>.previewWrapper { display: block; }</style><style>${tabOffCss}</style>`;
    await page.setContent(`<!doctype html>${tabOffCssTag}<div id="preview" class="previewWrapper"><div class="previewTab"><div class="previewObject"></div></div></div>`);
    assert.equal(await page.locator('#preview').evaluate(element => getComputedStyle(element).display), 'none', 'tabHoverEffect=false must hide tab previews independently');

    await page.setContent(`<!doctype html><style>.previewWrapper { display: block; position: fixed; left: 700px; top: 500px; }</style><style>${compileConditions(cssSources, { reducedMotion: false, tabHoverEffect: true, linkHoverEffect: true, objectMentionHoverEffect: true })}</style><a id="blob-source" href="blob://text.sfx/neon">Blob source</a><div id="preview" class="previewWrapper"><div class="previewObject"></div></div>`);
    await page.locator('#blob-source').hover();
    assert.equal(await page.locator('#preview').evaluate(element => getComputedStyle(element).display), 'none', 'blob links must remain outside the preview toggles');
  } finally {
    if (context) await context.close();
    fs.rmSync(testRoot, { recursive: true, force: true });
  }
});
