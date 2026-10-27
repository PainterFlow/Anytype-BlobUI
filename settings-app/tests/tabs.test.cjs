const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const ui = path.resolve(__dirname, '../ui');

test('shared tabs animate their indicator and keep the last tab inside its horizontal strip', async () => {
  const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  try {
    const page = await browser.newPage({ viewport: { width: 800, height: 650 } });
    await page.setContent(`<div class="page-content" style="height:300px;max-width:360px;margin-top:80px">
      <div style="height:120px"></div>
      <div id="tabs" class="advanced-tabs" role="tablist">${Array.from({ length: 5 }, (_, index) => `<button role="tab" id="tab-${index}" aria-controls="panel-${index}" aria-selected="${index === 0}">Tab ${index + 1}</button>`).join('')}</div>
      ${Array.from({ length: 5 }, (_, index) => `<section id="panel-${index}" style="height:600px">Panel ${index + 1}</section>`).join('')}
      </div>`);
    await page.addStyleTag({ path: path.join(ui, 'styles.css') });
    await page.addScriptTag({ path: path.join(ui, 'tabs.js') });
    await page.evaluate(() => {
      window.tabs = window.BlobUI.Tabs.attach({
        container: document.getElementById('tabs'),
        buttons: [...document.querySelectorAll('[role="tab"]')],
        panels: [...document.querySelectorAll('section')],
      });
      document.querySelector('.page-content').scrollTop = 80;
      document.getElementById('tab-0').focus({ preventScroll: true });
    });
    assert.match(await page.locator('#tabs').evaluate(node => getComputedStyle(node, '::after').transitionProperty), /transform/);
    assert.equal(await page.locator('#tabs').evaluate(node => getComputedStyle(node, '::after').display), 'block');
    const movement = await page.evaluate(() => {
      const strip = document.getElementById('tabs');
      const before = new DOMMatrixReadOnly(getComputedStyle(strip, '::after').transform).m41;
      window.tabs.select(4, { focus: true });
      const start = new DOMMatrixReadOnly(getComputedStyle(strip, '::after').transform).m41;
      return { before, start, target: document.getElementById('tab-4').offsetLeft };
    });
    assert.ok(movement.target > movement.before);
    assert.ok(movement.start < movement.target, 'The underline animates instead of jumping to the last tab');
    await page.waitForFunction(() => {
      const strip = document.getElementById('tabs');
      const current = new DOMMatrixReadOnly(getComputedStyle(strip, '::after').transform).m41;
      return Math.abs(current - document.getElementById('tab-4').offsetLeft) < 1;
    });
    await page.keyboard.press('Home');
    for (let repeat = 0; repeat < 3; repeat++) {
      await page.keyboard.press('End');
      assert.equal(await page.locator('#tab-4').getAttribute('aria-selected'), 'true');
      assert.equal(await page.locator('.page-content').evaluate(node => node.scrollTop), 0);
      const geometry = await page.locator('#tabs').evaluate(node => {
        const tab = document.getElementById('tab-4');
        return { scroll: node.scrollLeft, left: tab.offsetLeft, right: tab.offsetLeft + tab.offsetWidth, width: node.clientWidth,
          indicatorLeft: parseFloat(node.style.getPropertyValue('--tab-left')), indicatorWidth: parseFloat(node.style.getPropertyValue('--tab-width')) };
      });
      assert.ok(geometry.scroll > 0);
      assert.ok(geometry.left >= geometry.scroll - 1 && geometry.right <= geometry.scroll + geometry.width + 1);
      assert.equal(geometry.indicatorLeft, geometry.left);
      assert.equal(geometry.indicatorWidth, geometry.right - geometry.left);
      assert.equal(await page.locator('section:not([hidden])').count(), 1);
      await page.keyboard.press('Home');
      assert.equal(await page.locator('#tab-0').getAttribute('aria-selected'), 'true');
      assert.equal(await page.locator('#tabs').evaluate(node => node.scrollLeft), 0);
    }
    await page.locator('#tabs').evaluate(node => { node.scrollLeft = node.scrollWidth; });
    await page.locator('#tab-4').click();
    await page.setViewportSize({ width: 700, height: 650 });
    assert.equal(await page.locator('#tab-4').getAttribute('aria-selected'), 'true');
    const recoveredFocus = await page.evaluate(() => {
      const field = document.createElement('input');
      document.getElementById('panel-4').append(field);
      field.focus({ preventScroll: true });
      window.tabs.select(0);
      const focused = document.activeElement.id;
      window.tabs.select(4);
      return focused;
    });
    assert.equal(recoveredFocus, 'tab-0', 'Focus leaves a hidden panel without scrolling it into view');
    const preserved = await page.evaluate(() => {
      const scroller = document.querySelector('.page-content');
      scroller.scrollTop = 100;
      window.tabs.refresh();
      window.tabs.select(4);
      return scroller.scrollTop;
    });
    assert.equal(preserved, 100, 'Reselecting or refreshing does not undo user scrolling');
    await page.evaluate(() => { window.tabs.setEnabled(false); });
    assert.equal(await page.locator('section:not([hidden])').count(), 5);
    await page.evaluate(() => { window.tabs.setEnabled(true); });
    assert.equal(await page.locator('#panel-4').isVisible(), true);
    await page.emulateMedia({ reducedMotion: 'reduce' });
    assert.equal(await page.locator('#tabs').evaluate(node => getComputedStyle(node, '::after').transitionDuration), '0s');
    await page.evaluate(() => window.tabs.destroy());
  } finally { await browser.close(); }
});

test('real settings tabs keep their width and open panels at the top across height changes', async () => {
  const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const { pathToFileURL } = require('node:url');
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true, ignoreDefaultArgs: ['--hide-scrollbars'] });
  try {
    const page = await browser.newPage();
    await page.addInitScript(() => {
      // A new browser with a mocked bridge cannot read or alter a native credential.
      window.__TAURI__ = { core: { invoke: async command => {
        if (command === 'get_app_state') return { ready: true, onboardingComplete: true, theme: 'dark', hasSavedApiKey: true, settings: { settingsV21Enabled: true } };
        if (command === 'get_updater_state') return { automaticChecksEnabled: false, installDirectory: 'fixture' };
        if (command === 'get_calendar_api_availability') return 'available';
        if (command === 'configure_connection' || command === 'reveal_api_key') throw Error('Credential operations are forbidden in layout tests');
        return {};
      } } };
    });
    await page.goto(pathToFileURL(path.join(ui, 'index.html')).href);
    await page.waitForFunction(() => window.appReady);
    for (const size of [{ width: 800, height: 650 }, { width: 920, height: 740 }, { width: 960, height: 740 }]) {
      await page.setViewportSize(size);
      for (const [nav, ids] of [
        ['preferences', ['preference-tab-0', 'preference-tab-4', 'preference-tab-0', 'preference-tab-4']],
        ['advanced', ['advanced-app-tab', 'advanced-api-tab', 'advanced-experiments-tab', 'advanced-app-tab', 'advanced-experiments-tab', 'advanced-api-tab']],
      ]) {
        await page.locator(`#${nav}-nav`).click();
        const states = [];
        for (const id of ids) {
          await page.locator(`#${id}`).click();
          states.push(await page.locator(`#${id}`).evaluate(button => {
            const strip = button.parentElement;
            const scroller = document.querySelector('.page-content');
            return {
              stripWidth: strip.getBoundingClientRect().width,
              buttonWidths: [...strip.querySelectorAll('[role="tab"]')].map(tab => tab.getBoundingClientRect().width),
              clientWidth: scroller.clientWidth,
              reserved: scroller.offsetWidth - scroller.clientWidth,
              overflowing: scroller.scrollHeight > scroller.clientHeight,
            };
          }));
        }
        assert.ok(states.some(state => state.overflowing), `${nav} includes a tall panel`);
        assert.ok(states.some(state => !state.overflowing), `${nav} includes a short panel`);
        assert.ok(states.every(state => state.reserved === 12), 'The visible-scrollbar test reserves the 12px gutter on short and tall panels');
        for (const state of states) {
          assert.equal(state.stripWidth, states[0].stripWidth, `${nav} strip width stays fixed at ${size.width}px`);
          assert.equal(state.clientWidth, states[0].clientWidth);
          assert.deepEqual(state.buttonWidths, states[0].buttonWidths);
        }
      }
    }
    await page.setViewportSize({ width: 800, height: 650 });
    await page.evaluate(() => document.querySelectorAll('.settings-page > section').forEach(panel => { panel.style.minHeight = '1400px'; }));
    for (const [nav, ids] of [
      ['preferences', ['preference-tab-0', 'preference-tab-1', 'preference-tab-2', 'preference-tab-3', 'preference-tab-4']],
      ['advanced', ['advanced-api-tab', 'advanced-app-tab', 'advanced-experiments-tab']],
    ]) {
      await page.locator('#' + nav + '-nav').click();
      for (const from of ids) {
        for (const to of ids.filter(id => id !== from)) {
          await page.locator('#' + from).evaluate(button => button.click());
          await page.locator('.page-content').evaluate(scroller => { scroller.scrollTop = scroller.scrollHeight; });
          assert.ok(await page.locator('.page-content').evaluate(scroller => scroller.scrollTop > 0));
          await page.locator('#' + to).evaluate(button => button.click());
          await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
          assert.equal(await page.locator('.page-content').evaluate(scroller => scroller.scrollTop), 0, from + ' to ' + to + ' opens at the top');
          assert.equal(await page.locator('#' + to).getAttribute('aria-selected'), 'true');
        }
      }
    }
    // Delayed content growth must not anchor a newly selected tall panel to its bottom.
    await page.locator('#preferences-nav').click();
    await page.locator('#preference-tab-4').evaluate(button => button.click());
    await page.evaluate(() => {
      const content = document.createElement('div'); content.style.height = '400px';
      document.getElementById('preference-panel-4').prepend(content);
    });
    await page.evaluate(() => new Promise(resolve => requestAnimationFrame(() => requestAnimationFrame(resolve))));
    assert.equal(await page.locator('.page-content').evaluate(scroller => scroller.scrollTop), 0);
    // Sidebar navigation shares the policy and must not scroll past the tab strip.
    await page.locator('#advanced-nav').click();
    await page.locator('#advanced-section-links').getByRole('button', { name: 'App Settings', exact: true }).click();
    assert.equal(await page.locator('.page-content').evaluate(scroller => scroller.scrollTop), 0);
  } finally { await browser.close(); }
});
