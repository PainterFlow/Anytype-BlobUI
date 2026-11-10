const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { test } = require('node:test');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');

const workspace = path.resolve(__dirname, '../..');
const chrome = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const skip = !fs.existsSync(chrome) && 'Chrome required';

test('BlobUI text effects and blob markers work in Anytype chat bubbles', { skip }, async () => {
  const browser = await chromium.launch({ executablePath: chrome, headless: true });
  try {
    const page = await browser.newPage({ colorScheme: 'dark', reducedMotion: 'no-preference' });
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    const css = fs.readFileSync(path.join(workspace, 'Dev/50-text-effects.css'), 'utf8');
    await page.setContent(`<!doctype html><html class="themeDark"><head><style>
      :root { --color-shape-highlight-light-solid: #222; }
    </style><style>${css}</style></head><body>
      <div class="block blockChat"><div class="message isSelf"><div class="bubbleInner" id="chat">
        <a id="spoiler" href="blob://text.sfx/spoiler">Hidden text</a>
        <a id="jumping" href="blob://text.sfx/jumping">Jumping text</a>
        <a id="glitchy" href="blob://text.sfx/glitchy">Glitchy text</a>
        <a id="plain" href="blob://text.sfx/unknown">Plain marker</a>
      </div></div></div>
      <div class="pageMain"><div class="editor"><div class="blockText"><div class="editable">
        <a id="editor-jumping" href="blob://text.sfx/jumping">Editor text</a>
      </div></div></div></div>
    </body></html>`);
    await page.mouse.move(700, 700);

    const styles = await page.evaluate(() => {
      const get = id => getComputedStyle(document.getElementById(id));
      return {
        reducedMotion: matchMedia('(prefers-reduced-motion: reduce)').matches,
        spoiler: { color: get('spoiler').color, display: get('spoiler').display, background: get('spoiler').backgroundColor },
        jumping: { display: get('jumping').display, animation: get('jumping').animationName, pointer: get('jumping').pointerEvents },
        glitchy: { position: get('glitchy').position, animation: get('glitchy').animationName },
        plain: { decoration: get('plain').textDecorationLine, border: get('plain').borderBottomStyle, shadow: get('plain').boxShadow, pointer: get('plain').pointerEvents },
        editorAnimation: get('editor-jumping').animationName,
      };
    });

    assert.equal(styles.spoiler.color, 'rgba(0, 0, 0, 0)');
    assert.equal(styles.spoiler.display, 'inline-block');
    assert.notEqual(styles.spoiler.background, 'rgba(0, 0, 0, 0)');
    assert.equal(styles.jumping.display, 'inline-block');
    assert.equal(styles.jumping.animation, 'blobui-jumping-text', JSON.stringify(styles));
    assert.equal(styles.glitchy.position, 'relative');
    assert.match(styles.glitchy.animation, /blobui-glitch-jitter/);
    assert.equal(styles.plain.decoration, 'none');
    assert.equal(styles.plain.border, 'none');
    assert.equal(styles.plain.shadow, 'none');
    assert.equal(styles.plain.pointer, 'none');
    assert.equal(styles.jumping.pointer, 'none');
    assert.equal(styles.editorAnimation, styles.jumping.animation);

    await page.locator('#chat').hover();
    assert.notEqual(await page.locator('#spoiler').evaluate(el => getComputedStyle(el).color), 'rgba(0, 0, 0, 0)');
  } finally {
    await browser.close();
  }
});
