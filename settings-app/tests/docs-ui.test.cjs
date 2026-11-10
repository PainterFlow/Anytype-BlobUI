const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { loadSettings } = require('./frontend-fixture.cjs');

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

test('React release notes render sanitized Markdown and stop video media when the dialog closes', {
  skip: (!chromium || !chromePath) && 'Playwright and Google Chrome are required; set PLAYWRIGHT_MODULE and CHROME_PATH when they are not on the default paths',
  timeout: 60000,
}, async () => {
  const browser = await chromium.launch({ executablePath: chromePath, headless: true });
  try {
    const page = await browser.newPage();
    const pageErrors = [];
    page.on('pageerror', error => pageErrors.push(error.stack || error.message));
    const media = fs.readFileSync(path.join(workspace, 'settings-app/ui/cdn/kanban-compact-collapsing.mp4')).toString('base64');
    await page.addInitScript((video) => {
      window.__videoFixture = video;
      window.__TAURI__ = { core: { invoke: async (command) => {
        if (command === 'get_app_state') return { ready: true, onboardingComplete: true, theme: 'dark', settings: { settingsV21Enabled: false }, hasSavedApiKey: false, apiIp: '127.0.0.1', apiPort: 31009 };
        if (command === 'get_updater_state') return { installDirectory: 'C:/fixture', currentVersion: 'Dev+fixture', automaticChecksEnabled: false, automaticUpdatesEnabled: false };
        if (command === 'get_release_notes') return null;
        if (command === 'get_latest_release_notes') return { version: 'Dev+fixture', markdown: '@demo\n\n<script>window.releaseXss = true</script>', imageManifest: 'images: []\nvideos:\n  - id: demo\n    src: assets/changelogs/demo.mp4\n    alt: Demo video\n' };
        if (command === 'get_release_note_video' || command === 'get_release_note_history_video') return `data:video/mp4;base64,${window.__videoFixture}`;
        if (command === 'get_calendar_api_availability') return 'available';
        return {};
      } } };
    }, media);
    await loadSettings(page);
    await page.waitForTimeout(1000);
    const startup = await page.evaluate(() => ({ ready: window.appReady, bridge: typeof window.__TAURI__?.core?.invoke,
      status: document.getElementById('status-message')?.textContent, html: document.getElementById('settings-react-root')?.innerText.slice(0, 300) }));
    if (!startup.ready) throw new Error(JSON.stringify({ startup, pageErrors }));
    await page.locator('#help-button').click();
    await page.locator('#help-whats-new').click();
    await page.locator('#release-notes-backdrop').waitFor({ state: 'visible' });
    const player = page.locator('#release-notes-content .markdown-video');
    await page.waitForTimeout(1200);
    const mediaState = await page.evaluate(() => ({ html: document.getElementById('release-notes-content')?.innerHTML, calls: window.__TAURI__?.core?.calls }));
    if (!(await player.count())) throw new Error(JSON.stringify({ mediaState, pageErrors }));
    await page.waitForFunction(() => document.querySelector('#release-notes-content video')?.readyState >= 1);
    await page.waitForFunction(() => document.querySelector('#release-notes-content video')?.currentTime > 0.1);
    assert.equal(await page.evaluate(() => window.releaseXss), undefined, 'Unsafe Markdown scripts must not run.');
    const revocations = await page.evaluate(() => {
      window.revokedMedia = [];
      const revoke = URL.revokeObjectURL.bind(URL);
      URL.revokeObjectURL = value => { window.revokedMedia.push(value); revoke(value); };
      return true;
    });
    assert.equal(revocations, true);
    await page.keyboard.press('Escape');
    await page.locator('#release-notes-backdrop').waitFor({ state: 'hidden' });
    assert.equal(await player.locator('video').count(), 0, 'Closing release notes removes the video.');
    assert.equal(await page.evaluate(() => window.revokedMedia.length), 1, 'Closing release notes revokes the video URL.');
    assert.deepEqual(await page.evaluate(() => window.__TAURI__.core ? [] : ['bridge missing']), []);
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

test('Dev Guide and showcase tabs render offline in dark mode', {
  skip: (!chromium || !chromePath) && 'Playwright and Google Chrome are required; set PLAYWRIGHT_MODULE and CHROME_PATH when they are not on the default paths',
  timeout: 60000,
}, async () => {
  const testRoot = path.join(workspace, '.test-run', 'dev-guide-ui-integration');
  const screenshot = path.join(testRoot, 'dev-guide-preview.png');
  fs.mkdirSync(testRoot, { recursive: true });
  const browser = await chromium.launch({ executablePath: chromePath, headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  try {
    const page = await browser.newPage({ viewport: { width: 960, height: 740 } });
    const errors = [];
    const externalRequests = [];
    const oldDocumentationRequests = [];
    page.on('pageerror', error => errors.push(error.message));
    page.on('request', request => {
      if (!request.url().startsWith('http://blobui.test/') && !request.url().startsWith('blob:')) externalRequests.push(request.url());
    });
    await page.addInitScript(() => {
      window.__TAURI__ = { core: { invoke: async command => {
        if (command === 'get_app_state') return { ready: true, onboardingComplete: true, theme: 'dark', settings: { settingsV21Enabled: false }, hasSavedApiKey: false, apiIp: '127.0.0.1', apiPort: 31009 };
        if (command === 'get_updater_state') return { installDirectory: 'C:/fixture', currentVersion: 'Dev+fixture', automaticChecksEnabled: false, automaticUpdatesEnabled: false };
        if (command === 'get_release_notes' || command === 'get_latest_release_notes') return null;
        if (command === 'get_calendar_api_availability') return 'available';
        return {};
      } } };
    });
    await loadSettings(page);
    await page.route('http://blobui.test/docs/**', route => {
      oldDocumentationRequests.push(route.request().url());
      return route.abort();
    });
    await page.waitForTimeout(1000);
    const startup = await page.evaluate(() => ({ ready: window.appReady, bridge: typeof window.__TAURI__?.core?.invoke,
      status: document.getElementById('status-message')?.textContent, html: document.getElementById('settings-react-root')?.innerText.slice(0, 300) }));
    if (!startup.ready) throw new Error(JSON.stringify({ startup, pageErrors: errors }));

    await page.locator('#calendar-events-nav').click();
    await page.locator('#calendar-events-tab-2').click();
    await page.locator('.calendar-showcase-grid').waitFor();
    assert.equal(await page.locator('#calendar-events-tab-2').textContent(), 'Showcase');
    await page.getByRole('button', { name: 'Show 2 more events for June 12' }).click();
    assert.equal(await page.getByText('Pick up a parcel').count(), 1, 'Calendar overflow shows the remaining sample events.');
    await page.screenshot({ path: path.join(testRoot, 'calendar-showcase-dark.png') });

    await page.locator('#kanban-views-nav').click();
    await page.locator('#kanban-views-tab-2').click();
    const board = page.locator('.kanban-showcase-board');
    await board.waitFor();
    assert.equal(await page.locator('#kanban-views-tab-2').textContent(), 'Showcase');
    assert.equal(await board.getAttribute('data-layout'), 'default');
    assert.equal(await board.getAttribute('data-formula-display'), 'balanced');
    await page.screenshot({ path: path.join(testRoot, 'kanban-showcase-dark.png') });

    await page.locator('#help-button').click();
    await page.locator('#help-dev-guide').click();
    await page.waitForFunction(() => document.querySelector('#documentation-article h1')?.textContent === 'Developer guide');
    assert.equal(await page.locator('#documentation-navigation').count(), 0, 'The Dev Guide is a single page.');
    const guideText = await page.locator('#documentation-article').textContent();
    for (const section of ['Required tools', 'Install the tools', 'Build BlobUI', 'Release notes and Markdown', 'GitHub workflow']) {
      assert.ok(guideText.includes(section), `The Dev Guide includes ${section}.`);
    }
    const image = page.locator('#documentation-article img').first();
    await image.waitFor();
    assert.match(await image.getAttribute('src'), /^data:image\/png;base64,/);
    assert.equal(await page.locator('#documentation-content').evaluate(node => node.scrollWidth <= node.clientWidth), true);
    await page.locator('#documentation-content').evaluate(node => node.scrollTo({ top: 0, behavior: 'instant' }));
    await page.waitForTimeout(500);
    await page.screenshot({ path: screenshot });

    await page.goto('http://blobui.test/?window=component-catalog');
    await page.getByRole('main', { name: 'Component catalog' }).waitFor();
    await page.getByRole('button', { name: 'About color rules' }).focus();
    await page.waitForTimeout(500);
    await page.screenshot({ path: path.join(testRoot, 'component-catalog-help-dark.png') });
    await page.getByRole('tab', { name: 'Kanban' }).click();
    await page.locator('.component-catalog-card').filter({ has: page.getByRole('heading', { name: 'Showcase views', exact: true }) }).screenshot({ path: path.join(testRoot, 'component-catalog-showcase-dark.png') });
    assert.deepEqual(oldDocumentationRequests, [], 'The app no longer requests the old documentation articles.');
    assert.deepEqual(externalRequests, [], 'The Dev Guide and showcase previews should not request external resources.');
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
