const { loadSettings } = require('./frontend-fixture.cjs');
const test = require('node:test');
const assert = require('node:assert/strict');
const path = require('node:path');
const ui = path.resolve(__dirname, '../ui');

test('import and export preserve cancellation, refresh restored settings, and block stale saves', async () => {
  const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
  const browser = await chromium.launch({
    executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe',
    headless: true,
    ignoreDefaultArgs: ['--hide-scrollbars'],
  });
  try {
    const page = await browser.newPage({ viewport: { width: 1000, height: 850 }, reducedMotion: 'reduce' });
    const chooseExperiment = async label => {
      await page.locator('#background-experiment').click();
      await page.getByRole('option', { name: label, exact: true }).click();
    };
    const errors = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.addInitScript(() => {
      window.mockCalls = [];
      window.__TAURI__ = { core: { invoke: async (command, args) => {
        window.mockCalls.push({ command, args });
        if (command === 'get_app_state') {
          if (window.failReload) throw Error('Fixture reload failure');
          return { ready: true, onboardingComplete: true, theme: 'dark', settings: { fontFamily: window.imported ? 'Georgia' : 'Arial', backgroundExperiment: window.savedExperiment || 'disabled' }, hasSavedApiKey: false, apiIp: '127.0.0.1', apiPort: 31009 };
        }
        if (command === 'save_settings') { window.savedExperiment = args.settings.backgroundExperiment; return {}; }
        if (command === 'save_updater_settings') { window.savedStartupMode = args.settings.startupMode; return { ...args.settings, status: 'current' }; }
        if (command === 'get_updater_state') return { installDirectory: 'C:/fixture', startupMode: window.savedStartupMode || 'off', autoplayVideos: true, automaticUpdatesEnabled: false, automaticChecksEnabled: false, status: 'current', currentVersion: 'Dev+fixture' };
        if (command === 'get_calendar_api_availability') return 'available';
        if (command === 'get_keyboard_shortcuts') return [{ action: 'rebuildCss', code: 'KeyR', ctrl: false, alt: true, shift: false, meta: false }];
        if (command === 'load_kanban_formulas') return { configuration: { version: 2, display: 'balanced', definitions: [] }, calculatedAt: null };
        if (command === 'load_calendar_rules') return { version: 1, types: window.imported ? [{ spaceId: 'space', typeKey: 'task', default: { mode: 'none' }, overrides: [] }] : [] };
        if (command === 'discover_kanban_views') return { views: [] };
        if (command === 'get_release_notes' || command === 'get_latest_release_notes') return null;
        if (command === 'get_anytype_version') return 'Anytype 1.2.3';
        if (command === 'select_blob_backup_destination' || command === 'select_blob_backup_folder') {
          if (window.nextFolderPromise) return window.nextFolderPromise;
          return null;
        }
        if (window.allowWrites && command === 'create_blob_backup') return 'C:/fixture/export/BlobUI backup';
        if (window.allowWrites && command === 'import_blob_backup') {
          window.imported = true;
          window.failReload = !!window.failNextReload;
          return null;
        }
        if (command === 'configure_connection' || command === 'reveal_api_key' || command === 'import_blob_backup' || command === 'create_blob_backup') {
          throw Error('This cancellation test must not perform a native or data operation.');
        }
        return {};
      } } };
    });
    await loadSettings(page);
    await page.waitForFunction(() => window.appReady && document.getElementById('create-blob-backup-button')?.disabled === false);
    await page.locator('#advanced-nav').click();
    await page.locator('#advanced-app-tab').click();

    assert.equal(await page.locator('#window-background').isHidden(), true);
    await page.locator('#window-close').click();
    assert.equal(await page.evaluate(() => window.mockCalls.at(-1).command), 'hide_window');
    assert.equal(await page.locator('#advanced-app-panel #start-on-login').isVisible(), true);
    assert.equal(await page.locator('#startup-mode').count(), 0);
    await page.locator('#advanced-experiments-tab').click();
    assert.equal(await page.locator('#background-experiment').getAttribute('value'), 'disabled');
    await chooseExperiment('Variant 1');
    await page.locator('#window-close').click();
    assert.equal(await page.evaluate(() => window.mockCalls.at(-1).command), 'quit_app');
    assert.equal(await page.locator('#window-background').isVisible(), true);
    await page.locator('#window-background').focus();
    await page.keyboard.press('Enter');
    assert.equal(await page.evaluate(() => window.mockCalls.filter(call => call.command === 'run_in_background').length), 1);
    await page.locator('#advanced-app-tab').click();
    assert.equal(await page.locator('#start-on-login').isVisible(), true);
    assert.equal(await page.locator('#startup-mode').count(), 0);
    await page.locator('#discard-button').click();
    await page.locator('#discard-confirmation-undo').click();
    await page.waitForFunction(() => document.getElementById('window-background').hidden && !document.getElementById('settings-shell').inert);
    await page.locator('#advanced-experiments-tab').click();
    await chooseExperiment('Variant 2');
    await page.locator('#window-close').click();
    assert.equal(await page.evaluate(() => window.mockCalls.at(-1).command), 'quit_app');
    assert.equal(await page.locator('#advanced-experiments-panel #startup-mode, #advanced-experiments-panel #start-on-login').count(), 0);
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.savedExperiment === 'autostart-background' && document.getElementById('save-button').hidden);
    assert.equal(await page.locator('#api-key-removal-backdrop').isHidden(), true, 'An experiment-only save does not ask to remove a missing API key.');
    await page.locator('#advanced-app-tab').click();
    await page.locator('#startup-mode').click();
    await page.getByRole('option', { name: 'Autostart in background', exact: true }).click();
    await page.locator('#advanced-experiments-tab').click();
    await page.locator('#advanced-app-tab').click();
    assert.match(await page.locator('#startup-mode').textContent(), /Autostart in background/);
    await page.locator('#discard-button').click();
    await page.locator('#discard-confirmation-undo').click();
    await page.waitForFunction(() => document.getElementById('startup-mode').textContent.includes("Don't autostart") && !document.getElementById('settings-shell').inert);
    await page.locator('#startup-mode').click();
    await page.getByRole('option', { name: 'Autostart in background', exact: true }).click();
    await page.locator('#save-button').click();
    await page.waitForFunction(() => window.savedStartupMode === 'background' && document.getElementById('save-button').hidden);
    assert.equal(await page.evaluate(() => window.mockCalls.findLast(call => call.command === 'save_updater_settings').args.settings.startupMode), 'background');
    await page.locator('#advanced-experiments-tab').click();
    await chooseExperiment('Variant 1');
    await page.locator('#advanced-app-tab').click();
    assert.equal(await page.locator('#start-on-login').isChecked(), true);
    await page.locator('#advanced-experiments-tab').click();
    await chooseExperiment('Variant 2');
    await page.locator('#advanced-app-tab').click();
    assert.match(await page.locator('#startup-mode').textContent(), /Autostart in background/, 'Changing variants preserves the saved background startup mode.');
    await page.locator('#startup-mode').click();
    await page.getByRole('option', { name: "Don't autostart", exact: true }).click();
    await page.locator('#discard-button').click();
    await page.locator('#discard-confirmation-undo').click();
    await page.waitForFunction(() => document.getElementById('startup-mode').textContent.includes('Autostart in background') && !document.getElementById('settings-shell').inert);

    assert.equal(await page.locator('#advanced-app-panel > section').count(), 5);
    assert.equal(await page.locator('#advanced-app-panel > section').first().getAttribute('aria-labelledby'), 'app-general-heading');
    assert.equal(await page.locator('.app-backup-row').count(), 2);
    assert.equal(await page.locator('.app-backup-row .app-backup-description').count(), 2);
    assert.equal(await page.locator('.app-general-settings .settings-list').count(), 0);
    assert.equal(await page.locator('#blob-backup-status').count(), 1);
    await page.setViewportSize({ width: 700, height: 850 });
    await page.locator('#blob-backup-status').evaluate(node => { node.textContent = 'C:/fixture/' + 'long-path/'.repeat(40); });
    assert.equal(await page.locator('.app-backup-box').evaluate(node => node.scrollWidth <= node.clientWidth), true);
    await page.locator('#blob-backup-status').evaluate(node => { node.textContent = ''; });
    await page.locator('#create-blob-backup-button').focus();
    assert.equal(await page.locator('#create-blob-backup-button').evaluate(node => node === document.activeElement), true);
    await page.setViewportSize({ width: 1000, height: 850 });
    await page.locator('.app-backup-box').scrollIntoViewIfNeeded();
    await page.screenshot({ path: path.resolve(__dirname, '../../.test-run/app-settings-dark.png'), fullPage: true });
    const ordering = await page.evaluate(() => {
      const before = (left, right) => Boolean(document.getElementById(left).compareDocumentPosition(document.getElementById(right)) & Node.DOCUMENT_POSITION_FOLLOWING);
      return before('updates-heading', 'backup-heading') && before('backup-heading', 'uninstall-button');
    });
    assert.equal(ordering, true);
    assert.equal(await page.locator('#create-blob-backup-button').isEnabled(), true);
    assert.equal(await page.locator('#import-blob-backup-button').isEnabled(), true);
    assert.equal(await page.locator('#create-blob-backup-button').textContent(), 'Export');
    assert.equal(await page.locator('#import-blob-backup-button').textContent(), 'Import');

    const savesBeforeBackup = await page.evaluate(() => window.mockCalls.filter(call => call.command === 'save_settings').length);
    await page.evaluate(() => {
      window.nextFolderPromise = new Promise(resolve => { window.resolveNextFolder = resolve; });
    });
    await page.locator('#create-blob-backup-button').click();
    await page.waitForFunction(() => window.mockCalls.some(call => call.command === 'select_blob_backup_destination'));
    assert.equal(await page.locator('#create-blob-backup-button').isDisabled(), true, 'backup actions lock while a native picker is open');
    await page.keyboard.press('Alt+r');
    assert.equal(await page.evaluate(() => window.mockCalls.filter(call => call.command === 'save_settings').length), savesBeforeBackup, 'rebuild shortcut cannot save while exporting');
    await page.evaluate(() => window.resolveNextFolder(null));
    await page.waitForFunction(() => document.getElementById('create-blob-backup-button').disabled === false);
    assert.equal(await page.locator('#blob-backup-status').textContent(), '');
    assert.equal(await page.evaluate(() => window.mockCalls.some(call => call.command === 'create_blob_backup')), false);

    await page.evaluate(() => { window.nextFolderPromise = null; });
    await page.locator('#import-blob-backup-button').click();
    await page.waitForFunction(() => window.mockCalls.some(call => call.command === 'select_blob_backup_folder'));
    assert.equal(await page.locator('#import-blob-backup-button').isEnabled(), true);
    assert.equal(await page.locator('#blob-import-backdrop').isHidden(), true, 'cancelled import does not open the restore confirmation');
    assert.equal(await page.evaluate(() => window.mockCalls.some(call => call.command === 'import_blob_backup')), false);

    await page.evaluate(() => {
      window.allowWrites = true;
      window.nextFolderPromise = Promise.resolve('C:/fixture/export');
    });
    await page.locator('#create-blob-backup-button').click();
    await page.waitForFunction(() => document.getElementById('blob-backup-status').textContent.includes('Backup exported:'));
    await page.locator('#import-blob-backup-button').click();
    await page.locator('#blob-import-confirm').click();
    await page.waitForFunction(() => document.getElementById('blob-backup-status').textContent.includes('Backup imported:'));
    assert.equal(await page.locator('#font-family').inputValue(), 'Georgia', 'import reloads the restored settings');
    assert.equal(await page.evaluate(() => window.BlobUI.CalendarPage.read().types[0].default.mode), 'none', 'import reloads the restored calendar rules');
    assert.equal(await page.locator('#save-button').isHidden(), true, 'restored settings become the clean baseline');

    await page.evaluate(() => { window.failNextReload = true; });
    await page.locator('#import-blob-backup-button').click();
    await page.locator('#blob-import-confirm').click();
    await page.waitForFunction(() => document.getElementById('blob-import-error').textContent.includes('Settings could not reload'));
    await page.locator('#blob-import-cancel').click();
    await page.keyboard.press('Alt+r');
    assert.equal(await page.locator('#create-blob-backup-button').isDisabled(), true);
    assert.equal(await page.evaluate(() => window.mockCalls.filter(call => call.command === 'save_settings').length), savesBeforeBackup, 'failed reload blocks rebuild shortcuts from saving stale settings');
    assert.deepEqual(errors, []);
  } finally {
    await browser.close();
  }
});
