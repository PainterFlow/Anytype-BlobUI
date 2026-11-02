const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { chromium } = require(process.env.PLAYWRIGHT_MODULE || 'playwright');
const root = path.resolve(__dirname, '../../.test-run/kanban-live');
const card = id => '<div class="card" data-id="' + id + '"></div>';
const column = (id, cards) => '<div class="column" id="' + id + '"><div class="head"><div class="sides"><div class="side left"><span>Category</span></div><div class="side right"><button class="add">Add</button></div></div></div><div class="body"><div class="bg">' + cards + '<div class="card add" data-id="one"></div></div></div></div>';
const board = (view, id) => '<div id="block-board"><div data-root-id="object-a"><div id="view-item-board-' + view + '" class="viewItem active"></div></div><div class="viewContent viewBoard"><div id="columns">' + column(id, card('one') + card('two')) + column(id + '-empty', '') + column(id + '-unselected', card('unselected')) + '</div></div></div>';
test('v3 results follow all rendered card anchors and isolate views in dark mode', async () => {
  const browser = await chromium.launch({ executablePath: process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe', headless: true });
  try {
    const page = await browser.newPage({ colorScheme: 'dark' });
    for (const display of ['balanced', 'spacious']) {
      const css = fs.readFileSync(path.join(root, 'v3-' + display + '.css'), 'utf8');
      await page.setContent('<style>:root {color-scheme:dark}body {background:#171717;color:#ddd}.column{width:250px}.sides{display:flex}' + css + '</style>' + board('view-a', 'matched') + board('view-b', 'other'));
      const result = id => page.locator('#' + id + ' > .head .side.left').evaluate(el => getComputedStyle(el, '::after').content);
      const populated = await result('matched'); const empty = await result('matched-empty');
      assert.notEqual(populated, empty);
      assert.match(await result('matched-unselected'), /3/);
      assert.equal(await result('other'), 'none');
      await page.locator('#matched .card[data-id="one"]:not(.add)').evaluate(el => el.remove());
      assert.equal(await result('matched'), populated, 'every saved card can anchor');
      await page.locator('#matched .card[data-id="two"]').evaluate(el => el.remove());
      assert.equal(await result('matched'), empty, 'add placeholders never anchor');
      await page.locator('#matched-empty .body .bg').evaluate(el => el.insertAdjacentHTML('afterbegin', '<div class="card" data-id="two"></div>'));
      assert.equal(await result('matched-empty'), populated, 'delayed insertion or a moved snapshot card uses its saved category');
      await page.locator('#matched-empty .card:not(.add)').evaluate(el => el.remove());
      assert.equal(await result('matched-empty'), empty);
      await page.emulateMedia({ reducedMotion: 'reduce' });
      assert.equal(await page.locator('#matched .side.left').evaluate(el => getComputedStyle(el, '::after').transitionDuration), '0s');
      await page.screenshot({path: path.join(root, 'v3-' + display + '.png')});
    }
  } finally { await browser.close(); }
});
