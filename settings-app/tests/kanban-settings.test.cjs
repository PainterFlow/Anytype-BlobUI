const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const test = require('node:test');
const workspace = path.resolve(__dirname, '../..');
let chromium;
try { chromium = require(process.env.PLAYWRIGHT_MODULE || 'playwright').chromium; } catch {}
const chromePath = [process.env.CHROME_PATH, 'C:/Program Files/Google/Chrome/Application/chrome.exe'].filter(Boolean).find(p => fs.existsSync(p));

test('Kanban picker uses themed options with keyboard navigation, Save, and Undo', {
  skip: (!chromium || !chromePath) && 'Playwright and Chrome required',
}, async () => {
  const root = path.join(workspace, '.test-run/kanban-settings');
  fs.mkdirSync(path.join(root, 'temp'), { recursive: true });
  process.env.TEMP = process.env.TMP = path.join(root, 'temp');
  const context = await chromium.launchPersistentContext(path.join(root, 'profile'), { headless: true, executablePath: chromePath });
  try {
    const page = await context.newPage();
    await page.addInitScript(() => {
      window.__saves = [];
      window.__TAURI__ = { core: { invoke: async (name, args) => {
        if (name === 'get_app_state') return { ready: true, theme: document.documentElement.dataset.theme || 'dark', onboardingComplete: true, hasSavedApiKey: false, settings: { fontFamily: 'Segoe UI', codeFontFamily: 'Consolas', accentColor: 'Default', accentMode: 'native', kanbanViewStyle: 'default', collapsedCategories: false } };
        if (name === 'save_settings') { window.__saves.push(args.settings); return { calendarStatus: 'cached' }; }
        if (name === 'get_theme') return 'dark';
        return null;
      } } };
    });
    await page.goto(pathToFileURL(path.join(workspace, 'settings-app/ui/index.html')).href);
    await page.waitForFunction(() => window.appReady);
    const trigger = page.locator('#kanban-view-style');
    const menu = page.locator('#kanban-style-options');
    assert.equal(await trigger.evaluate(el => el.tagName), 'BUTTON');
    await trigger.focus();
    await page.keyboard.press('ArrowDown');
    assert.equal(await menu.isVisible(), true);
    assert.equal(await page.getByRole('option', { name: /Default/ }).evaluate(el => el === document.activeElement), true);
    await page.keyboard.press('End');
    await page.keyboard.press('Enter');
    assert.equal(await trigger.getAttribute('value'), 'compact');
    await menu.waitFor({ state: 'hidden' });
    assert.equal(await menu.isVisible(), false);
    assert.equal(await trigger.evaluate(el => el === document.activeElement), true);
    assert.equal(await page.locator('#kanban-style-value').innerText(), 'Compact');
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.__saves.length === 1);
    assert.equal(await page.evaluate(() => window.__saves[0].kanbanViewStyle), 'compact');
    await trigger.click();
    await page.keyboard.press('Home');
    await page.keyboard.press('ArrowDown');
    await page.keyboard.press('Enter');
    assert.equal(await trigger.getAttribute('value'), 'balanced');
    await page.evaluate(() => window.BlobUI.discardChanges());
    assert.equal(await trigger.getAttribute('value'), 'compact');
    await trigger.click();
    await page.keyboard.press('Escape');
    assert.equal(await trigger.getAttribute('aria-expanded'), 'false');
    await menu.waitFor({ state: 'hidden' });
    assert.equal(await menu.isVisible(), false);
    assert.equal(await trigger.evaluate(el => el === document.activeElement), true);
    await trigger.click();
    await page.keyboard.press('Tab');
    await menu.waitFor({ state: 'hidden' });
    assert.equal(await menu.isVisible(), false);
    assert.equal(await page.locator('#formula-display').evaluate(el => el === document.activeElement), true);
    const formulaOptions = page.locator('#formula-display-options [role="option"]');
    assert.equal(await formulaOptions.count(), 2);
    assert.deepEqual(await formulaOptions.allTextContents(), ['Balanced', 'Spacious']);
    await page.keyboard.press('Tab');
    assert.equal(await page.locator('#collapsed-categories').evaluate(el => el === document.activeElement), true);
    const section = page.locator('section[aria-labelledby="kanban-heading"]');
    for (const theme of ['dark', 'light']) {
      await page.evaluate(value => { document.documentElement.dataset.theme = value; }, theme);
      await section.scrollIntoViewIfNeeded();
      await section.screenshot({ path: path.join(root, `settings-${theme}.png`) });
      await trigger.click();
      assert.equal(await menu.getByRole('option', { name: /Compact/ }).getAttribute('aria-selected'), 'true');
      assert.equal(await menu.locator('.kanban-option-description').count(), 0);
      const selected = menu.getByRole('option', { name: 'Compact', exact: true });
      assert.equal(await selected.evaluate(el => el.getBoundingClientRect().height), 34);
      const selectedBackground = await selected.evaluate(el => getComputedStyle(el).backgroundColor);
      const otherOption = menu.getByRole('option', { name: 'Balanced', exact: true });
      await otherOption.hover();
      assert.equal(await otherOption.evaluate(el => getComputedStyle(el).backgroundColor), selectedBackground);
      assert.equal(await trigger.evaluate(el => getComputedStyle(el).borderTopWidth), '0px');
      if (theme === 'dark') assert.notEqual(await menu.evaluate(el => getComputedStyle(el).backgroundColor), 'rgb(255, 255, 255)');
      await menu.screenshot({ path: path.join(root, `picker-${theme}.png`) });
      await page.locator('#kanban-heading').click();
      await menu.waitFor({ state: 'hidden' });
    assert.equal(await menu.isVisible(), false);
    }
    // Reopening during the exit transition must invalidate its pending hide.
    const rapidReopen = await page.evaluate(() => {
      const button = document.getElementById('kanban-view-style');
      const popup = document.getElementById('kanban-style-options');
      button.click();
      button.click();
      const closing = { state: popup.dataset.state, inert: popup.inert, hidden: popup.hidden };
      button.click();
      return closing;
    });
    assert.equal(rapidReopen.state, 'closing');
    assert.equal(rapidReopen.inert, true);
    assert.equal(rapidReopen.hidden, false, 'Closing must animate before hiding');
    await page.waitForTimeout(180);
    assert.equal(await trigger.getAttribute('aria-expanded'), 'true');
    assert.equal(await menu.isVisible(), true, 'A stale close timer must not hide the reopened popup');
    assert.equal(await menu.evaluate(el => el.inert), false);
    assert.equal(await menu.evaluate(el => getComputedStyle(el).transitionDuration.split(',')[0]), '0.14s');
    await page.keyboard.press('Escape');
    await menu.waitFor({ state: 'hidden' });
    for (const mode of ['app', 'system']) {
      if (mode === 'app') await page.evaluate(() => { document.documentElement.dataset.reducedMotion = 'true'; });
      else await page.emulateMedia({ reducedMotion: 'reduce' });
      const reduced = await page.evaluate(() => {
        const button = document.getElementById('kanban-view-style');
        const popup = document.getElementById('kanban-style-options');
        button.click();
        const duration = getComputedStyle(popup).transitionDuration;
        button.click();
        return { hidden: popup.hidden, duration };
      });
      assert.equal(reduced.hidden, true, 'Reduced Motion closes immediately');
      assert.equal(reduced.duration, '0s');
      await page.evaluate(() => { document.documentElement.dataset.reducedMotion = 'false'; });
    }
    await page.emulateMedia({ reducedMotion: 'no-preference' });
    await trigger.click();
    await page.locator('#accessibility-nav').click();
    await menu.waitFor({ state: 'hidden' });
    assert.equal(await menu.isVisible(), false);
  } finally { await context.close(); }
});
