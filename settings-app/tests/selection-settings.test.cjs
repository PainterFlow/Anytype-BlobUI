const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const test = require('node:test');
const workspace = path.resolve(__dirname, '../..');
let chromium;
try { chromium = require(process.env.PLAYWRIGHT_MODULE || 'playwright').chromium; } catch {}
const chrome = 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const skip = (!chromium || !fs.existsSync(chrome)) && 'Playwright and Chrome required';

async function browserFixture(name, run) {
  const root = path.join(workspace, '.test-run', name);
  fs.mkdirSync(path.join(root, 'temp'), { recursive: true });
  const context = await chromium.launchPersistentContext(path.join(root, 'profile'), {
    executablePath: chrome, headless: true,
    env: { ...process.env, TEMP: path.join(root, 'temp'), TMP: path.join(root, 'temp') },
  });
  try { await run(await context.newPage(), root); } finally { await context.close(); }
}

test('Include selections sits inside the accent box and supports migration, Save, Undo and native mode', { skip }, async () => {
  await browserFixture('selection-settings', async (page, root) => {
    await page.addInitScript(() => {
      if (!sessionStorage.getItem('selection-fixture-started')) {
        localStorage.removeItem('selection-fixture');
        sessionStorage.setItem('selection-fixture-started', 'true');
      }
      const saved = JSON.parse(localStorage.getItem('selection-fixture') || '{}');
      window.__saves = [];
      window.__TAURI__ = { core: { invoke: async (name, args) => {
        if (name === 'get_app_state') return { ready: true, theme: 'dark', onboardingComplete: true, settings: {
          fontFamily: 'Segoe UI', codeFontFamily: 'Consolas', accentMode: 'native', accentColor: 'Default', ...saved,
        } };
        if (name === 'save_settings') {
          window.__saves.push(args.settings);
          localStorage.setItem('selection-fixture', JSON.stringify(args.settings));
          return { calendarStatus: 'off' };
        }
        if (name === 'get_theme') return 'dark';
        return null;
      } } };
    });
    await page.goto(pathToFileURL(path.join(workspace, 'settings-app/ui/index.html')).href);
    await page.waitForFunction(() => window.appReady);
    const toggle = page.locator('#include-selections');
    assert.equal(await toggle.isChecked(), true);
    assert.equal(await toggle.isEnabled(), true, 'Native accent must not disable this control');
    assert.equal(await toggle.evaluate(el => el.closest('.settings-list') === document.querySelector('#native-accent').closest('.settings-list')), true);
    assert.equal(await toggle.evaluate(el => !!(el.compareDocumentPosition(document.querySelector('#native-accent')) & Node.DOCUMENT_POSITION_FOLLOWING)), true);
    await toggle.focus();
    await page.keyboard.press('Space');
    assert.equal(await toggle.isChecked(), false);
    assert.equal(await page.locator('#save-button').isVisible(), true);
    await page.evaluate(() => window.BlobUI.discardChanges());
    assert.equal(await toggle.isChecked(), true);
    assert.equal(await page.locator('#save-button').isVisible(), false);
    await toggle.uncheck();
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.__saves.length === 1);
    assert.equal(await page.evaluate(() => window.__saves[0].includeSelections), false);
    await page.reload();
    await page.waitForFunction(() => window.appReady);
    assert.equal(await toggle.isChecked(), false);
    await toggle.check();
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.__saves.length === 1);
    assert.equal(await page.evaluate(() => window.__saves[0].includeSelections), true);
    await page.locator('#native-accent').uncheck();
    await page.locator('#accent-value').fill('#27a879');
    const appSelection = () => page.locator('#accent-value').evaluate(el => getComputedStyle(el, '::selection').backgroundColor);
    const customSelection = await appSelection();
    assert.equal(await page.locator('html').getAttribute('data-include-selections'), 'true');
    await toggle.uncheck();
    assert.notEqual(await appSelection(), customSelection, 'Disabling selections restores the Settings default');
    await page.evaluate(() => window.BlobUI.discardChanges());
    assert.equal(await toggle.isChecked(), true);
    assert.equal(await page.locator('html').getAttribute('data-include-selections'), 'true');
    await page.locator('#native-accent').uncheck();
    await page.locator('#accent-value').fill('#27a879');
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.__saves.length === 2);
    await page.reload();
    await page.waitForFunction(() => window.appReady);
    assert.equal(await appSelection(), customSelection, 'Settings text selection follows the saved accent after reload');
    for (const theme of ['dark', 'light']) {
      await page.evaluate(value => document.documentElement.dataset.theme = value, theme);
      await page.locator('.accent-group').screenshot({ path: path.join(root, `accent-${theme}.png`) });
    }
  });
});

test('Real compiler output recolors selection indicators and preserves unrelated native colors', { skip }, async () => {
  await browserFixture('selection-dom', async page => {
    const nativePath = path.join(workspace, '.test-run/anytype-native.css');
    // Small native fixture keeps this runnable without the optional full cache.
    const native = fs.existsSync(nativePath) ? fs.readFileSync(nativePath, 'utf8') : `
      :root { --color-system-accent-100:#377aff; --color-system-accent-50:#699bff; --color-system-selection:rgba(55,122,255,.25); --color-system-drop-zone:rgba(55,122,255,.25); --color-control-accent:#252525; }
      ::selection { background-color:var(--color-system-selection); }
      .selectionTarget.isSelectionSelected::after, .listObject .table .row.isSelectionSelected, .selectionFrame.tableSelection, .upsellBanner { background-color:var(--color-system-selection); }
      .selectionFrame { border:2px solid var(--color-system-accent-50); }
      .cell.isEditing .cellContent { box-shadow:0 0 0 2px var(--color-system-accent-50); }
      .dropTarget.isOver.middle { background:var(--color-system-drop-zone)!important; }
      .dropTarget.isOver.top { box-shadow:0 -2px 0 var(--color-control-accent); }
      .layout.active, .inputWithLabel.isFocused { border:2px solid var(--color-system-accent-100); }
      code[class*=language-]::selection { background:#b3d4fc; }
      .textLayer br::selection { background:transparent; }
      .blocks .block.blockTable .handle .inner::after { content:''; background-color:var(--color-system-accent-100); }
      .blocks .block.blockTable .handle .inner { width:2px; }
      .blocks .block.blockTable .handle.isActive .inner { width:10px; }
      .blocks .block.blockTable .cell.isHighlightedCell .icon.menu .inner { box-shadow:0 0 0 2px var(--color-system-accent-50); }
      .menus .menu.menuDataviewText:not(.withTitle).show #input { box-shadow:0 0 0 2px var(--color-system-accent-50); }
      .block.blockChat .message.isSelf .bubbleInner ::selection { background:#9bbcff; }
    `;
    const html = `<div class="blocks"><div class="block blockTable">
      <div class="handle handleRow" id="row-handle"><div class="inner" id="row-handle-inner"></div></div>
      <div class="handle handleColumn" id="column-handle"><div class="inner" id="column-handle-inner"></div></div>
      <div class="cell isHighlightedCell"><div class="icon menu"><div class="inner" id="cell-menu"></div></div></div>
      </div></div>
      <div class="menus"><div class="menu menuDataviewText show"><div id="input" data-property-editor="menu">Text property</div></div>
      <div class="menu menuDataviewText isInline show"><div id="input" data-property-editor="inline">Inline text property</div></div></div>
      <div id="selection-rect"></div>
      <div class="selectionTarget isSelectionSelected" id="block">Block</div>
      <div class="listObject"><div class="table"><div class="row isSelectionSelected" id="record">Record</div></div></div>
      <div class="blocks"><div class="block blockTable"><div id="selectionFrameContainer"><div class="selectionFrame tableSelection" id="frame"></div></div></div></div>
      <div class="cell isEditing"><div class="cellContent" id="edit">Cell</div></div>
      <div class="dropTarget isOver middle" id="drop"></div>
      <div class="dropTarget isOver top" id="line"></div>
      <div class="menus"><div class="menu menuDataviewViewLayout"><div class="layouts"><div class="layout active" id="choice"></div></div></div></div>
      <div class="inputWithLabel isFocused" id="focus"></div>
      <div class="upsellBanner" id="notice"></div>
      <pre class="language-js"><code class="language-js" id="code">const value = 1;</code></pre>
      <div class="blocks"><div class="block blockChat"><div class="message isSelf"><div class="bubbleInner"><span id="chat">Selected chat</span></div></div></div></div>
      <div class="textLayer"><span id="pdf">PDF text</span><br id="pdf-break"></div>
      <input id="caret" value="Typing" />`;
    for (const theme of ['light', 'dark']) {
      for (const accent of ['native', 'preset', 'custom', 'alpha']) {
        const values = {};
        for (const enabled of [false, true]) {
          const css = fs.readFileSync(path.join(workspace, `.test-run/selections-compiled/${accent}-${enabled}.css`), 'utf8');
          await page.setContent(`<html class="${theme === 'dark' ? 'themeDark' : ''}"><head><style>${native}</style><style>${css}</style></head><body>${html}</body></html>`);
          values[enabled] = await page.evaluate(() => {
            function style(id, prop, pseudo) { return getComputedStyle(document.getElementById(id), pseudo).getPropertyValue(prop); }
            return {
              rowHandle: style('row-handle-inner', 'background-color', '::after'),
              columnHandle: style('column-handle-inner', 'background-color', '::after'),
              cellMenu: style('cell-menu', 'box-shadow'),
              menuProperty: getComputedStyle(document.querySelector('[data-property-editor="menu"]')).boxShadow,
              inlineProperty: getComputedStyle(document.querySelector('[data-property-editor="inline"]')).boxShadow,
              block: style('block', 'background-color', '::after'),
              record: style('record', 'background-color'), frame: style('frame', 'border-top-color'),
              frameFill: style('frame', 'background-color'), edit: style('edit', 'box-shadow'),
              drop: style('drop', 'background-color'), line: style('line', 'box-shadow'),
              choice: style('choice', 'border-top-color'), focus: style('focus', 'border-top-color'),
              notice: style('notice', 'background-color'), code: style('code', 'background-color', '::selection'),
              chat: style('chat', 'background-color', '::selection'), pdf: style('pdf', 'background-color', '::selection'),
              pdfBreak: style('pdf-break', 'background-color', '::selection'), caret: style('caret', 'caret-color'),
            };
          });
          for (const axis of ['row', 'column']) {
            const inner = page.locator('#' + axis + '-handle-inner');
            const collapsedColor = await inner.evaluate(el => getComputedStyle(el, '::after').backgroundColor);
            assert.equal(await inner.evaluate(el => getComputedStyle(el).width), '2px');
            await page.locator('#' + axis + '-handle').evaluate(el => el.classList.add('isActive'));
            assert.equal(await inner.evaluate(el => getComputedStyle(el).width), '10px');
            assert.equal(await inner.evaluate(el => getComputedStyle(el, '::after').backgroundColor), collapsedColor);
          }
        }
        for (const key of ['notice', 'pdfBreak', 'caret']) assert.equal(values.true[key], values.false[key], `${theme}/${accent} changed ${key}`);
        if (accent !== 'native') {
          for (const key of ['rowHandle', 'columnHandle', 'cellMenu', 'menuProperty', 'inlineProperty', 'block', 'record', 'frame', 'frameFill', 'edit', 'drop', 'line', 'choice', 'focus', 'code', 'chat', 'pdf']) {
            assert.notEqual(values.true[key], values.false[key], `${theme}/${accent} failed to recolor ${key}`);
          }
        }
        assert.equal(values.true.block, values.true.record);
        assert.equal(values.true.block, values.true.frameFill);
        assert.equal(values.true.code, values.true.chat);
      }
    }
    await page.emulateMedia({ forcedColors: 'active' });
    assert.notEqual(await page.locator('#code').evaluate(el => getComputedStyle(el, '::selection').backgroundColor), 'rgba(0, 0, 0, 0)');
  });
});
