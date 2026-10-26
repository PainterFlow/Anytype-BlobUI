const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const ui = path.resolve(__dirname, '../ui');

test('mounted screens retain accessible controls, ordering and initial state', async () => {
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true });
  try {
    const page = await browser.newPage();
    const html = fs.readFileSync(path.join(ui, 'index.html'), 'utf8');
    await page.setContent(html.replace(/<script\b[^>]*>[\s\S]*?<\/script>/g, ''));
    for (const match of html.matchAll(/<script src="\.\/([^"]+)"/g)) {
      await page.addScriptTag({ path: path.join(ui, match[1]) });
      if (match[1] === 'screens.js') break;
    }
    const result = await page.evaluate(() => {
      const get = id => document.getElementById(id);
      const before = (a, b) => Boolean(get(a).compareDocumentPosition(get(b)) & Node.DOCUMENT_POSITION_FOLLOWING);
      const ids = [...document.querySelectorAll('[id]')].map(node => node.id);
      const switches = ['include-selections', 'native-accent', 'collapsed-categories', 'calendar-colors-enabled', 'reduced-motion', 'tab-hover-effect', 'link-hover-effect', 'object-mention-hover-effect', 'settings-v21-enabled', 'start-on-login', 'automatic-updates-enabled', 'automatic-checks-enabled'];
      const fields = ['connection-key', 'connection-port', 'advanced-api-key', 'advanced-api-ip', 'advanced-api-port'];
      const problems = [];
      for (const node of document.querySelectorAll('[aria-controls], [aria-labelledby], [aria-describedby]')) {
        for (const attr of ['aria-controls', 'aria-labelledby', 'aria-describedby']) {
          for (const id of (node.getAttribute(attr) || '').split(/\s+/).filter(Boolean)) if (!get(id)) problems.push(id);
        }
      }
      const first = get('font-family'); first.focus(); first.value = 'Draft'; first.setSelectionRange(1, 3);
      BlobUI.Screens.mount();
      return {
        uniqueIds: ids.length === new Set(ids).size, problems,
        mounted: !document.querySelector('template[data-ui-mount]') && Object.keys(BlobUI.ScreenDefinitions).every(id => BlobUI.Screens.get(id)),
        switches: switches.every(id => get(id)?.type === 'checkbox' && get(id).getAttribute('role') === 'switch' && get(id).labels.length > 0),
        visibility: fields.every(id => get(id).labels.length > 0 && get(id).parentElement.id === id + '-field' && get(id).nextElementSibling.id === id + '-visibility' && get(id + '-visibility').hasAttribute('aria-pressed')),
        ordering: before('preferences-nav', 'accessibility-nav') && before('accessibility-nav', 'advanced-nav') && before('install-directory', 'start-on-login') && before('start-on-login', 'updates-heading') && before('updates-heading', 'backup-heading') && before('backup-heading', 'uninstall-button') && before('help-whats-new', 'help-documentation') && before('help-documentation', 'help-keybinds'),
        numeric: get('connection-port').inputMode === 'numeric' && get('advanced-api-port').inputMode === 'numeric' && get('advanced-api-ip').inputMode === 'decimal',
        folder: get('install-directory').readOnly && get('install-directory').getAttribute('aria-haspopup') === 'dialog',
        calendar: get('calendar-colors-enabled').checked && get('calendar-colors-enabled').disabled,
        hidden: get('advanced-app-panel').hidden && get('release-notes-current-tag').hidden && get('release-notes-latest-tag').hidden,
        modals: ['uninstall', 'blob-import', 'api-key-removal', 'discard-confirmation', 'help-error'].every(id => get(id + '-dialog').getAttribute('role') === 'alertdialog' && get(id + '-dialog').getAttribute('aria-modal') === 'true'),
        undo: get('api-key-removal-undo').textContent === 'Undo' && get('discard-confirmation-undo').textContent === 'Undo' && !get('api-key-removal-dismiss') && !get('api-key-removal-cancel'),
        preview: Boolean(document.querySelector('.space-sidebar-preview[inert][role="img"]')) && ['image/png', 'image/jpeg', 'image/webp'].every(type => get('banner-image-input').accept.includes(type)),
        stable: first === get('font-family') && first.value === 'Draft' && first.selectionStart === 1 && first.selectionEnd === 3,
      };
    });
    assert.deepEqual(result.problems, [], 'ARIA references must resolve in the mounted DOM');
    for (const [name, value] of Object.entries(result)) if (name !== 'problems') assert.equal(value, true, name);
  } finally { await browser.close(); }
});
