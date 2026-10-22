const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const test = require('node:test');
const workspace = path.resolve(__dirname, '../..');
let chromium;
try { chromium = require(process.env.PLAYWRIGHT_MODULE || 'playwright').chromium; } catch {}
const chromePath = [process.env.CHROME_PATH, 'C:/Program Files/Google/Chrome/Application/chrome.exe'].filter(Boolean).find(p => fs.existsSync(p));

function compile(source, style, collapsed) {
  const flags = { kanbanStyled: style !== 'default', kanbanCompact: style === 'compact', kanbanBalanced: style === 'balanced', collapsedCategories: collapsed };
  return source.replace(/\/\* @blobui-if (\w+) \*\/([\s\S]*?)\/\* @blobui-endif \1 \*\//g, (_, flag, css) => flags[flag] ? css : '');
}

// Minimal native geometry from Anytype 0.57.4. Preserve the ancestor specificity
// and property constraints that the theme must override.
const native = `
* { box-sizing: border-box; }
body { margin: 20px; font: 14px/20px sans-serif; }
.viewContent { width: 560px; overflow-x: auto; }
.blocks .block.blockDataview .viewContent.viewBoard .columns { display:flex; gap:0 8px; }
.blocks .block.blockDataview .viewContent.viewBoard .column { width:262px; flex-shrink:0; display:flex; flex-direction:column; padding:0 0 16px; }
.blocks .block.blockDataview .viewContent.viewBoard .column .head { padding:8px 16px 0; }
.head { height:40px; background: #ddd; position:relative; }
.head > .bgColor { position:absolute; inset:0; background:#ccc; border-radius:4px 4px 0 0; z-index:-1; }
.blocks .block.blockDataview .viewContent.viewBoard .column .body .bg { padding:7px; }
.blocks .block.blockDataview .viewContent.viewBoard .card { width:246px; display:flex; flex-direction:column; margin:0 auto 8px; }
.blocks .block.blockDataview .viewContent.viewBoard .card .cardContent > .inner { padding:16px; display:flex; flex-direction:column; align-items:flex-start; }
.blocks .block.blockDataview .viewContent.viewBoard .card .cellContent:not(.isName) { height:24px; white-space:nowrap; }
.cellContent .wrap { white-space:nowrap; }
.cellContent .name { overflow:hidden; text-overflow:ellipsis; white-space:nowrap; }
.cellContent.isName { display:flex; gap:6px; line-height:20px; align-items:flex-start; }
.cellContent.isName button { flex-shrink:0; }
.cardContent { border:1px solid #888; }
.tagItem { display:inline-flex; align-items:center; height:24px; line-height:24px; padding:0 6px; border:1px solid #888; border-radius:12px; }
.tagItem.isSmall { height:20px; line-height:20px; }
.tagItem .inner { display:inline-block; width:100%; white-space:nowrap; overflow:hidden; text-overflow:ellipsis; }
.cellContent.c-select .over { height:18px; }
`;

function board(inline) {
  const card = `<div class="card isTask" draggable="true">${inline ? '' : '<div class="selectionTarget">'}<div class="cardContent"><div class="inner">
    <div class="cellContent isName"><button aria-label="Complete task">✓</button><span class="name">WS 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 51, 52, 53, 54 with enough extra text to exceed the title's two-line limit</span></div>
    <div class="cellContent property-a"><div class="name">Today</div></div>
    <div class="cellContent c-select property-b"><div class="wrap"><span class="over"><span class="tagItem isSmall"><span class="inner">High</span></span></span></div></div>
    <div class="cellContent c-select"><div class="wrap"><span class="over"><span class="tagItem isSmall"><span class="inner">LongUnbrokenPropertyValueThatMustFitInsideTheCardWithoutHorizontalOverflow</span></span></span></div></div>
    <div class="cellContent isEmpty"><div class="empty">Select date</div></div>
  </div></div>${inline ? '' : '</div>'}</div>`;
  const column = content => `<div class="column withColor"><div class="head">Category<div class="bgColor"></div></div><div class="body"><div class="bg">${content}<button class="card add">Add task</button><div class="bgColor"></div></div></div></div>`;
  return `<div class="blocks"><div class="block blockDataview ${inline ? 'isInline' : 'noPlus'}"><div class="viewContent viewBoard"><div id="columns" class="columns">${column(card + card)}${column('')}${column(card)}</div></div></div></div>`;
}

test('Kanban styles preserve geometry, wrap properties, and expand collapsed columns', {
  skip: (!chromium || !chromePath) && 'Playwright and Chrome required',
}, async () => {
  const root = path.join(workspace, '.test-run/kanban-dom');
  fs.mkdirSync(path.join(root, 'temp'), { recursive: true });
  process.env.TEMP = process.env.TMP = path.join(root, 'temp');
  const source = fs.readFileSync(path.join(workspace, 'Dev/41-kanban.css'), 'utf8');
  const context = await chromium.launchPersistentContext(path.join(root, 'profile'), { executablePath: chromePath, headless: true });
  try {
    const page = await context.newPage();
    for (const inline of [false, true]) for (const style of ['default', 'balanced', 'compact']) for (const collapsed of [false, true]) {
      await page.setContent(`<style>${native}\n${compile(source, style, collapsed)}</style>${board(inline)}`);
      await page.mouse.move(1000, 700);
      const inner = page.locator('.cardContent > .inner').first();
      assert.equal(await inner.evaluate(el => getComputedStyle(el).paddingTop), style === 'default' ? '16px' : style === 'balanced' ? '8px' : '6px');
      assert.equal(await page.locator('.column').first().evaluate(el => el.getBoundingClientRect().width), style === 'compact' ? 220 : style === 'balanced' ? 246 : 262);
      assert.equal(await page.locator('.card').first().evaluate(el => el.getBoundingClientRect().width), style === 'compact' ? 204 : style === 'balanced' ? 230 : 246);
      if (collapsed) {
        assert.equal(await page.locator('.body').first().evaluate(el => el.getBoundingClientRect().height), 0);
        assert.equal(await page.locator('.head > .bgColor').first().evaluate(el => getComputedStyle(el).borderBottomLeftRadius), '8px');
        assert.equal(await page.locator('.body').first().evaluate(el => getComputedStyle(el).transitionDuration.split(',')[0]), '0.18s');
        await page.locator('.head').first().hover();
        await page.waitForFunction(() => getComputedStyle(document.querySelector('.body')).opacity === '1');
        assert.ok(await page.locator('.body').first().evaluate(el => el.getBoundingClientRect().height) > 0);
        await page.mouse.move(1000, 700);
        await page.waitForFunction(() => document.querySelector('.body').getBoundingClientRect().height === 0);
        await page.keyboard.press('Tab');
        await page.waitForFunction(() => getComputedStyle(document.querySelector('.body')).opacity === '1');
        assert.ok(await page.locator('.column').first().evaluate(el => el.matches(':focus-within')));
        assert.ok(await page.locator('.body').first().evaluate(el => el.getBoundingClientRect().height) > 0);
      }
      if (style === 'compact') {
        const geometry = await inner.evaluate(el => {
          const a = el.querySelector('.property-a').getBoundingClientRect();
          const b = el.querySelector('.property-b').getBoundingClientRect();
          return { sameRow: Math.abs((a.top + a.bottom) / 2 - (b.top + b.bottom) / 2) < 1, fits: el.scrollWidth <= el.clientWidth };
        });
        assert.ok(geometry.sameRow, 'Short properties must share a row');
        assert.ok(geometry.fits, 'Long property values must fit the card');
        const tag = await page.locator('.property-b .tagItem').first().evaluate(el => {
          const label = el.querySelector('.inner').getBoundingClientRect();
          const pill = el.getBoundingClientRect();
          return { delta: Math.abs((label.top + label.bottom) / 2 - (pill.top + pill.bottom) / 2), height: pill.height };
        });
        assert.ok(tag.delta < 1, 'Tag text must be vertically centered');
        assert.ok(tag.height <= 22, 'Short property tags must retain their compact native height');
        const truncation = await inner.evaluate(el => {
          const title = el.querySelector('.isName .name');
          const titleBox = title.getBoundingClientRect();
          const nextProperty = el.querySelector('.property-a').getBoundingClientRect();
          const longTag = el.querySelectorAll('.tagItem .inner')[1];
          return {
            titleHeight: titleBox.height,
            titleClipped: title.scrollHeight > title.clientHeight,
            titleOverflow: getComputedStyle(title).overflow,
            nextPropertyTop: nextProperty.top,
            titleBottom: titleBox.bottom,
            tagEllipsis: getComputedStyle(longTag).textOverflow,
            tagClipped: longTag.scrollWidth > longTag.clientWidth,
            tagHeight: longTag.getBoundingClientRect().height,
          };
        });
        assert.ok(truncation.titleHeight <= 40, 'Title must stop at two lines');
        assert.ok(truncation.titleClipped, 'A long title must be clipped');
        assert.equal(truncation.titleOverflow, 'hidden');
        assert.ok(truncation.nextPropertyTop >= truncation.titleBottom, 'Title must not overlap properties');
        assert.equal(truncation.tagEllipsis, 'ellipsis');
        assert.ok(truncation.tagClipped, 'Long tag text must truncate');
        assert.equal(truncation.tagHeight, 20);
      }
    }
    await page.emulateMedia({ colorScheme: 'dark', reducedMotion: 'reduce' });
    await page.setContent(`<style>${native}\n${compile(source, 'compact', true)}</style>${board(true)}`);
    assert.equal(await page.locator('.body').first().evaluate(el => getComputedStyle(el).transitionDuration.split(',')[0]), '0s');
    await page.locator('.head').first().hover();
    await page.screenshot({ path: path.join(root, 'compact.png'), fullPage: true });
    const touch = await context.browser().newContext({ hasTouch: true, isMobile: true });
    const touchPage = await touch.newPage();
    await touchPage.setContent(`<style>${native}\n${compile(source, 'default', true)}</style>${board(true)}`);
    assert.ok(await touchPage.locator('.body').first().evaluate(el => el.getBoundingClientRect().height) > 0);
    await touch.close();
  } finally { await context.close(); }
});
