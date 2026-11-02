const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { pathToFileURL } = require('node:url');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const workspace = path.resolve(__dirname, '../..');

test('Settings sends loaded revisions, advances after saves, and retains conflicting drafts', async () => {
  const root = path.join(workspace, '.test-run/configuration-revisions');
  fs.mkdirSync(path.join(root, 'temp'), { recursive: true });
  process.env.TEMP = process.env.TMP = path.join(root, 'temp');
  const browser = await chromium.launch({ executablePath: 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true });
  const page = await browser.newPage({ colorScheme: 'dark' });
  await page.addInitScript(() => {
    window.__calls = [];
    window.__TAURI__ = { core: { invoke: async command => {
      window.__calls.push(command);
      if (command === 'get_app_state') return { ready: true, theme: 'dark', onboardingComplete: true, hasSavedApiKey: true, settings: {} };
      if (command === 'get_calendar_api_availability') return 'available';
      if (command === 'load_kanban_formulas') return { revision: 'formula-original', configuration: { version: 2, display: 'balanced', definitions: [] } };
      if (command === 'load_calendar_rules') return { revision: 'calendar-original', version: 1, types: [] };
      if (command === 'discover_kanban_views') return { views: [] };
      if (command === 'discover_calendar_colors') return { types: [], views: [] };
      return null;
    } } };
  });
  try {
    await page.goto(pathToFileURL(path.join(workspace, 'settings-app/ui/index.html')).href);
    await page.waitForFunction(() => window.appReady && window.__calls.includes('load_calendar_rules'));
    const result = await page.evaluate(async () => {
      const UI = window.BlobUI, sent = [];
      const save = async (command, args) => { sent.push([command, args.revision]); return { revision: command === 'save_kanban_formulas' ? 'formula-next' : 'calendar-next', info: [] }; };
      const formulas = UI.FormulaPage.read(), calendar = UI.CalendarPage.read();
      await UI.FormulaPage.save(save, formulas);
      await UI.CalendarPage.save(save, calendar, {});
      await UI.FormulaPage.save(save, formulas);
      await UI.CalendarPage.save(save, calendar, {});
      const before = [UI.FormulaPage.read(), UI.CalendarPage.read()];
      const reject = async () => { throw Error('Saved configuration changed. Reload Settings before saving your draft.'); };
      let failures = 0;
      try { await UI.FormulaPage.save(reject, formulas); } catch { failures++; }
      try { await UI.CalendarPage.save(reject, calendar, {}); } catch { failures++; }
      return { sent, before, after: [UI.FormulaPage.read(), UI.CalendarPage.read()], failures };
    });
    assert.deepEqual(result.sent, [['save_kanban_formulas', 'formula-original'], ['save_calendar_rules', 'calendar-original'], ['save_kanban_formulas', 'formula-next'], ['save_calendar_rules', 'calendar-next']]);
    assert.equal(result.failures, 2);
    assert.deepEqual(result.after, result.before);
    assert.equal(result.after[1].revision, undefined);
  } finally { await browser.close(); }
});
