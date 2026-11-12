const assert = require('node:assert/strict');
const fs = require('node:fs/promises');
const path = require('node:path');
const test = require('node:test');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const { loadSettings } = require('./frontend-fixture.cjs');

const workspace = path.resolve(__dirname, '../..');
const screenshotDirectory = path.join(workspace, '.test-run/custom-theme-appshots');
const chromePath = process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';
const userAccentCss = `:root, html, html.themeDark { --color-blob-accent: #DE50DE; }`;
const darkThemeCss = `
html, html.themeDark {
  --color-bg-primary: #191918;
  --color-bg-secondary: #1c1c1b;
  --color-text-primary: #e1e1e1;
  --color-text-secondary: #a3a3a3;
  --color-text-tertiary: #5c5c5c;
  --color-shape-highlight-dark: rgba(255, 255, 255, .11);
  --color-shape-highlight-medium: rgba(255, 255, 255, .05);
  --color-shape-secondary: #292929;
  --color-light-grey: #414141;
  --color-blob-accent: #de50de;
}`;
test('custom theme previews update BlobUI Settings and save with dark appshots', {
  skip: !require('node:fs').existsSync(chromePath) && 'Google Chrome is required for the isolated appshot check.',
  timeout: 60000,
}, async () => {
  const browser = await chromium.launch({ executablePath: chromePath, headless: true });
  try {
    const page = await browser.newPage({ viewport: { width: 1280, height: 900 }, colorScheme: 'dark' });
    await page.addInitScript(({ userAccentCss, darkThemeCss }) => {
      window.__activePreviewTheme = 'system';
      window.__activeThemeCss = userAccentCss;
      window.__calls = [];
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        window.__calls.push({ command, args });
        if (command === 'get_app_state') return {
          ready: true,
          onboardingComplete: true,
          theme: 'dark',
          previewTheme: window.__activePreviewTheme,
          themeCss: window.__activeThemeCss,
          customThemes: [
            { id: 'dark-mode-plus.css', name: 'Dark Mode+', author: 'PainterFlow11', mainColor: '#191918' },
            { id: 'oled.css', name: 'OLED', author: 'PainterFlow11', mainColor: '#000000' },
          ],
          themeDirectory: 'Settings/themes/built-in-themes',
          settings: {
            fontFamily: 'Inter', codeFontFamily: 'Consolas', accentColor: '#DE50DE', enableCustomThemes: false,
            accentMode: 'native', accentPreset: '', basedTheme: 'auto',
            themePreset: 'dark-mode-plus.css', themeColor: '#191918',
            includeSelections: true, reducedMotion: false, tabHoverEffect: false,
            linkHoverEffect: false, objectMentionHoverEffect: false,
            kanbanViewStyle: 'default', collapsedCategories: false,
            calendarColorsEnabled: true, sidebarResizeMode: 'off',
          },
        };
        if (command === 'save_settings') {
          window.__activePreviewTheme = 'dark';
          window.__activeThemeCss = darkThemeCss;
          window.__savedSettings = args.settings;
          return { calendarStatus: 'cached', info: [], themeCss: darkThemeCss, previewTheme: 'dark' };
        }
        if (command === 'get_updater_state') return {};
        if (command === 'get_anytype_theme') return 'dark';
        if (command === 'get_anytype_version') return 'Fixture';
        if (command === 'load_kanban_formulas') return { configuration: { version: 2, display: 'balanced', definitions: [] } };
        if (command === 'load_calendar_rules') return { version: 1, types: [] };
        if (command === 'get_calendar_api_availability') return 'missing';
        return null;
      } } };
    }, { userAccentCss, darkThemeCss });

    await loadSettings(page);
    await page.waitForFunction(() => window.appReady && document.documentElement.dataset.theme === 'dark');
    await page.locator('#preferences-nav').click();
    await page.locator('#preference-tab-0').click();
    await page.locator('#custom-themes-heading').waitFor({ state: 'visible' });

    const darkPreview = await page.evaluate(() => ({
      background: getComputedStyle(document.body).backgroundColor,
      titlebar: getComputedStyle(document.querySelector('.window-titlebar')).backgroundColor,
      updateText: getComputedStyle(document.getElementById('check-updates-button')).color,
      secondNavGap: getComputedStyle(document.querySelector('#sidebar-nav > .nav-group-label:not(:first-child)')).marginTop,
      isDark: document.documentElement.classList.contains('themeDark'),
    }));
    assert.equal(darkPreview.background, 'rgb(23, 23, 23)');
    assert.equal(darkPreview.titlebar, 'rgb(23, 23, 23)');
    assert.equal(darkPreview.updateText, 'rgb(225, 225, 225)');
    assert.equal(darkPreview.secondNavGap, '24px');
    assert.equal(darkPreview.isDark, true);

    await fs.mkdir(screenshotDirectory, { recursive: true });
    assert.equal(await page.locator('#enable-custom-themes').isChecked(), false);
    assert.equal(await page.locator('#based-theme-select .kanban-select-trigger').isDisabled(), true);
    assert.equal(await page.locator('#custom-theme-select .kanban-select-trigger').isDisabled(), true);
    assert.equal(await page.locator('#theme-color-value').isDisabled(), true);
    assert.equal(await page.locator('#theme-color-plane').getAttribute('aria-disabled'), 'true');
    await page.locator('.custom-themes-group').screenshot({ path: path.join(screenshotDirectory, 'custom-themes-disabled.png') });
    await page.locator('#enable-custom-themes').click();
    assert.equal(await page.locator('#enable-custom-themes').isChecked(), true);
    assert.equal(await page.locator('#based-theme-select .kanban-select-trigger').isDisabled(), false);
    assert.equal(await page.locator('#custom-theme-select .kanban-select-trigger').isDisabled(), false);
    assert.equal(await page.locator('#theme-color-value').isDisabled(), false);
    assert.equal(await page.locator('#theme-color-plane').getAttribute('aria-disabled'), 'false');
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.__savedSettings?.enableCustomThemes === true);
    assert.equal(await page.locator('body').evaluate(element => getComputedStyle(element).backgroundColor), 'rgb(25, 25, 24)');
    await page.screenshot({ path: path.join(screenshotDirectory, 'dark-mode-plus-settings.png'), fullPage: true });
    await page.locator('.custom-themes-group').screenshot({ path: path.join(screenshotDirectory, 'dark-mode-plus-custom-themes.png') });

    const basedThemeTrigger = page.locator('#based-theme-select .kanban-select-trigger');
    await basedThemeTrigger.press('ArrowDown');
    await page.locator('#based-theme-options').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#based-theme-options .kanban-select-option-description').count(), 0);
    assert.equal(await page.evaluate(() => getComputedStyle(document.activeElement).outlineStyle), 'none');
    await page.locator('.custom-themes-group').screenshot({ path: path.join(screenshotDirectory, 'based-theme-options.png') });
    await page.keyboard.press('Escape');

    const presetTrigger = page.locator('#custom-theme-select .kanban-select-trigger');
    await presetTrigger.press('ArrowDown');
    await page.locator('#custom-theme-options').waitFor({ state: 'visible' });
    assert.equal(await page.locator('#custom-theme-options .kanban-select-option-description').count(), 0);
    await page.locator('.custom-themes-group').screenshot({ path: path.join(screenshotDirectory, 'preset-theme-options.png') });
    await page.locator('#custom-theme-options [data-value="oled.css"]').click();
    await page.locator('#custom-theme-options').waitFor({ state: 'hidden' });
    await page.waitForFunction(() => document.getElementById('theme-color-value').value === '#000000');
    assert.equal(await page.locator('#theme-color-swatch').evaluate(element => getComputedStyle(element).backgroundColor), 'rgb(0, 0, 0)');
    await page.locator('.custom-theme-color-row').screenshot({ path: path.join(screenshotDirectory, 'oled-main-color.png') });
    assert.equal(await page.locator('#discard-button').isVisible(), true);
    await page.locator('#discard-button').click();
    await page.locator('#discard-confirmation-undo').click();
    await page.locator('#discard-confirmation-backdrop').waitFor({ state: 'hidden' });
    await page.waitForFunction(() => document.getElementById('theme-color-value').value === '#191918');
    assert.equal(await page.locator('#custom-theme-select .kanban-trigger-value').textContent(), 'Dark Mode+');

    await page.locator('#theme-color-value').fill('#202020');
    assert.equal(await page.locator('#discussion-fade').count(), 0, 'The footer fade is configured in theme CSS instead of Settings.');
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.__savedSettings?.themeColor === '#202020' && document.documentElement.dataset.theme === 'dark');
    const savedPreview = await page.evaluate(() => ({
      background: getComputedStyle(document.body).backgroundColor,
      isDark: document.documentElement.classList.contains('themeDark'),
      color: window.__savedSettings.themeColor,
      enabled: window.__savedSettings.enableCustomThemes,
    }));
    assert.equal(savedPreview.background, 'rgb(25, 25, 24)');
    assert.equal(savedPreview.isDark, true);
    assert.equal(savedPreview.color, '#202020');
    assert.equal(savedPreview.enabled, true);
  } finally {
    await browser.close();
  }
});
