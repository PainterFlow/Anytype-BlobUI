const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const vm = require('node:vm');
const workspace = path.resolve(__dirname, '../..');
const ui = path.join(workspace, 'settings-app/ui');
const parserContext = {};
vm.runInNewContext(fs.readFileSync(path.join(ui, 'markdown.js'), 'utf8'), parserContext);
const Markdown = parserContext.BlobUI.Markdown;

test('media manifests preserve images and validate video sections, aliases, and paths', () => {
  const yaml = 'images:\n  - id: picture\n    src: assets/changelogs/picture.png\nvideos:\n  - id: demo\n    src: assets/changelogs/demo.mp4\n    alt: Demo video\n';
  const parsed = Markdown.parseMediaManifest(yaml);
  assert.equal(parsed.videos.demo.alt, 'Demo video');
  assert.equal(Markdown.parseImageManifest(yaml).picture.src, 'assets/changelogs/picture.png');
  assert.deepEqual(Object.keys(Markdown.parseMediaManifest('images: []').videos), []);
  for (const invalid of [yaml.replace('demo.mp4', '../demo.mp4'), yaml.replace('demo.mp4', 'demo.webm'), yaml.replace('id: demo', 'id: picture'), yaml + 'videos: []\n', yaml.replace('assets/changelogs/demo.mp4', 'https://host/demo.mp4')]) {
    assert.throws(() => Markdown.parseMediaManifest(invalid));
  }
});

let chromium;
try { chromium = require(process.env.PLAYWRIGHT_MODULE || 'playwright').chromium; } catch {}
const chrome = path.join(process.env.PROGRAMFILES || 'C:/Program Files', 'Google/Chrome/Application/chrome.exe');

test('shared videos play, reveal controls, seek, respect autoplay, and clean up', {
  skip: (!chromium || !fs.existsSync(chrome)) && 'Playwright and Chrome required', timeout: 60000,
}, async () => {
  const root = path.join(workspace, '.test-run', 'video-player-' + process.pid);
  fs.mkdirSync(root, { recursive: true });
  const old = {};
  for (const key of ['APPDATA', 'LOCALAPPDATA', 'TEMP', 'TMP', 'TMPDIR']) {
    old[key] = process.env[key]; process.env[key] = path.join(root, key); fs.mkdirSync(process.env[key], { recursive: true });
  }
  let context;
  try {
    context = await chromium.launchPersistentContext(path.join(root, 'profile'), { executablePath: chrome, headless: true, viewport: { width: 960, height: 740 } });
    const page = await context.newPage();
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.setContent('<!doctype html><html><body style="margin:20px;overflow:auto;height:auto"><main id="docs" class="markdown-content" style="width:640px"></main><div style="height:1500px"></div><main id="notes" class="markdown-content" style="width:640px"></main></body></html>');
    for (const file of ['styles.css']) await page.addStyleTag({ path: path.join(ui, file) });
    for (const file of ['vendor/marked.umd.js', 'video-player.js', 'markdown.js']) await page.addScriptTag({ path: path.join(ui, file) });
    const source = 'data:video/mp4;base64,' + fs.readFileSync(path.join(ui, 'cdn/kanban-compact-collapsing.mp4')).toString('base64');
    const outcome = await page.evaluate(src => {
      window.testMedia = { demo: { src, alt: 'Kanban demonstration' } };
      window.BlobUI.Markdown.mount(document.querySelector('#docs'), '@demo\n\n`@demo`\n\n```\n@demo\n```\n\n![Second video](@demo)\n\n<video src="https://bad/video.mp4" autoplay></video>', { videos: window.testMedia });
      return { count: document.querySelectorAll('video').length, code: [...document.querySelectorAll('code')].map(node => node.textContent) };
    }, source);
    assert.equal(outcome.count, 2);
    assert.deepEqual(outcome.code.map(value => value.trim()), ['@demo', '@demo']);
    const first = page.locator('#docs .markdown-video').first();
    await page.waitForFunction(() => document.querySelector('#docs video')?.currentTime > 0.05);
    assert.equal(await first.locator('video').evaluate(video => video.muted), true);
    assert.equal(await first.locator('.markdown-video-controls').evaluate(node => getComputedStyle(node).opacity), '0');
    await first.hover();
    await page.waitForFunction(() => getComputedStyle(document.querySelector('.markdown-video-controls')).opacity === '1');
    await page.waitForFunction(() => getComputedStyle(document.querySelector('.markdown-video-controls')).opacity === '0', { timeout: 5000 });
    await first.focus();
    await page.keyboard.press('Space');
    assert.equal(await first.locator('video').evaluate(video => video.paused), true);
    await page.evaluate(() => scrollTo(0, 900));
    await page.evaluate(() => scrollTo(0, 0));
    await page.waitForTimeout(150);
    assert.equal(await first.locator('video').evaluate(video => video.paused), true, 'Manual pause persists after visibility changes');
    const pausedTime = await first.locator('video').evaluate(video => video.currentTime);
    await first.getByRole('button', { name: 'Forward 10 seconds', exact: true }).click();
    assert.ok(await first.locator('video').evaluate(video => video.currentTime >= 10));
    await first.getByRole('button', { name: 'Back 10 seconds', exact: true }).click();
    assert.ok(await first.locator('video').evaluate((video, expected) => Math.abs(video.currentTime - expected) < 0.1, pausedTime));
    await page.evaluate(() => BlobUI.VideoPlayer.setAutoplay(false));
    await first.getByRole('button', { name: 'Play', exact: true }).click();
    await page.waitForFunction(() => !document.querySelector('#docs video').paused);
    await first.getByRole('button', { name: 'Unmute', exact: true }).click();
    assert.equal(await first.locator('video').evaluate(video => video.muted), false);
    await page.evaluate(() => { document.querySelector('#docs').hidden = true; });
    await page.waitForFunction(() => document.querySelector('#docs video').paused);
    await page.evaluate(() => { document.querySelector('#docs').hidden = false; });
    await page.waitForTimeout(150);
    assert.equal(await first.locator('video').evaluate(video => video.paused), true);
    await page.evaluate(() => {
      window.revoked = [];
      const revoke = URL.revokeObjectURL.bind(URL);
      URL.revokeObjectURL = value => { window.revoked.push(value); revoke(value); };
      BlobUI.Markdown.mount(document.querySelector('#docs'), '# Replacement');
      BlobUI.Markdown.mount(document.querySelector('#notes'), '@demo', { videos: window.testMedia });
      document.querySelector('#notes').scrollIntoView();
    });
    assert.equal(await page.locator('#docs video').count(), 0);
    assert.equal(await page.evaluate(() => window.revoked.length), 2);
    const notes = page.locator('#notes .markdown-video');
    await page.waitForFunction(() => document.querySelector('#notes video').readyState >= 1);
    assert.equal(await notes.locator('video').evaluate(video => video.paused), true, 'Autoplay off applies to other markdown readers');
    await notes.scrollIntoViewIfNeeded();
    await page.evaluate(() => BlobUI.VideoPlayer.setAutoplay(true));
    await page.waitForFunction(() => !document.querySelector('#notes video').paused);
    await notes.focus();
    await page.waitForFunction(() => getComputedStyle(document.querySelector('#notes .markdown-video-controls')).opacity === '1');
    await notes.screenshot({ path: path.join(root, 'shared-video-controls.png') });
    await notes.getByRole('button', { name: 'Enter fullscreen', exact: true }).click();
    await page.waitForFunction(() => document.fullscreenElement !== null);
    await notes.getByRole('button', { name: 'Exit fullscreen', exact: true }).click();
    await page.waitForFunction(() => document.fullscreenElement === null);
    await notes.locator('video').evaluate(video => { video.currentTime = video.duration - 0.05; });
    await page.waitForFunction(() => document.querySelector('#notes video').ended);
    await page.evaluate(() => { BlobUI.VideoPlayer.setAutoplay(false); BlobUI.VideoPlayer.setAutoplay(true); });
    assert.equal(await notes.locator('video').evaluate(video => video.paused), true, 'Completed videos do not loop or restart automatically');
    await page.evaluate(() => {
      BlobUI.Markdown.mount(document.querySelector('#notes'), '@bad', { videos: { bad: { src: 'https://bad/video.mp4' } } });
    });
    await page.waitForFunction(() => document.querySelector('#notes .markdown-video-status').textContent.includes('unsupported'));
    assert.deepEqual(errors, []);
  } finally {
    if (context) await context.close();
    for (const key of Object.keys(old)) { if (old[key] === undefined) delete process.env[key]; else process.env[key] = old[key]; }
  }
});
