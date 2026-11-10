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
      .block.blockChat .message .bubbleInner { color: rgb(230, 230, 230); font-weight: 400; }
      .block.blockChat .message .bubbleInner a[href^="blob://"],
      .block.blockChat .message .bubbleInner a[href^="blob://"] * {
        color: rgb(128, 128, 128) !important;
        font-weight: 700 !important;
        -webkit-text-fill-color: rgb(128, 128, 128) !important;
      }
    </style><style>${css}</style></head><body>
      <div class="block blockChat"><div class="message isSelf"><div class="bubbleInner" id="chat">
        <a id="spoiler" href="blob://text.sfx/spoiler">Hidden text</a>
        <a id="jumping" href="blob://text.sfx/jumping"><span id="jumping-text">Jumping text</span></a>
        <a id="glitchy" href="blob://text.sfx/glitchy"><span id="glitchy-text">Glitchy text</span></a>
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
        jumping: { display: get('jumping').display, animation: get('jumping').animationName, pointer: get('jumping').pointerEvents, color: get('jumping').color, weight: get('jumping').fontWeight, fill: get('jumping').webkitTextFillColor },
        jumpingText: { color: get('jumping-text').color, weight: get('jumping-text').fontWeight, fill: get('jumping-text').webkitTextFillColor },
        glitchy: { position: get('glitchy').position, animation: get('glitchy').animationName, color: get('glitchy').color, weight: get('glitchy').fontWeight, fill: get('glitchy').webkitTextFillColor },
        glitchyText: { color: get('glitchy-text').color, weight: get('glitchy-text').fontWeight, fill: get('glitchy-text').webkitTextFillColor },
        plain: { decoration: get('plain').textDecorationLine, border: get('plain').borderBottomStyle, shadow: get('plain').boxShadow, pointer: get('plain').pointerEvents },
        editorAnimation: get('editor-jumping').animationName,
      };
    });

    assert.equal(styles.spoiler.color, 'rgba(0, 0, 0, 0)');
    assert.equal(styles.spoiler.display, 'inline-block');
    assert.notEqual(styles.spoiler.background, 'rgba(0, 0, 0, 0)');
    assert.equal(styles.jumping.display, 'inline-block');
    assert.equal(styles.jumping.animation, 'blobui-jumping-text', JSON.stringify(styles));
    for (const link of [styles.jumping, styles.glitchy]) {
      assert.equal(link.color, 'rgb(230, 230, 230)', JSON.stringify(styles));
      assert.equal(link.weight, '400', JSON.stringify(styles));
      assert.equal(link.fill, 'rgb(230, 230, 230)', JSON.stringify(styles));
    }
    for (const text of [styles.jumpingText, styles.glitchyText]) {
      assert.equal(text.color, 'rgb(230, 230, 230)', JSON.stringify(styles));
      assert.equal(text.weight, '400', JSON.stringify(styles));
      assert.equal(text.fill, 'rgb(230, 230, 230)', JSON.stringify(styles));
    }
    assert.equal(styles.glitchy.position, 'relative');
    assert.match(styles.glitchy.animation, /blobui-glitch-jitter/);
    assert.equal(styles.plain.decoration, 'none');
    assert.equal(styles.plain.border, 'none');
    assert.equal(styles.plain.shadow, 'none');
    assert.equal(styles.plain.pointer, 'none');
    assert.equal(styles.jumping.pointer, 'none');
    assert.equal(styles.editorAnimation, styles.jumping.animation);

    await page.locator('#glitchy').evaluate(el => {
      const bold = el.getAnimations().find(animation => animation.animationName === 'blobui-glitch-bold');
      if (bold) bold.currentTime = 4000;
    });
    assert.equal(await page.locator('#glitchy').evaluate(el => getComputedStyle(el).fontWeight), '400');

    await page.locator('#chat').hover();
    assert.notEqual(await page.locator('#spoiler').evaluate(el => getComputedStyle(el).color), 'rgba(0, 0, 0, 0)');
  } finally {
    await browser.close();
  }
});
