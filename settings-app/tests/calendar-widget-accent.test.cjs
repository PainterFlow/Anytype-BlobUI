const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('path');
const { execFileSync } = require('node:child_process');

const workspace = path.resolve(__dirname, '../..');
const chromePath = process.env.CHROME_PATH
  || ['C:/Program Files/Google/Chrome/Application/chrome.exe', 'C:/Program Files (x86)/Google/Chrome/Application/chrome.exe']
    .find(candidate => fs.existsSync(candidate));

test('widget calendar today number and event dot compute to the configured accent', {
  skip: !chromePath && 'Google Chrome is required; set CHROME_PATH if it is not installed in the default location',
}, () => {
  const cssPath = process.env.BLOBUI_CSS_PATH || path.join(workspace, 'Dev', '40-calendar.css');
  const css = fs.readFileSync(cssPath, 'utf8')
    .replace(/\/\*\s*@blobui-if-custom-accent\s*\*\//g, '')
    .replace(/\/\*\s*@blobui-endif-custom-accent\s*\*\//g, '')
    .replace(/<\/style/gi, '<\\/style');
  const testRoot = path.join(workspace, '.test-run', 'calendar-widget-accent');
  fs.mkdirSync(testRoot, { recursive: true });
  const profile = path.join(testRoot, 'chrome-profile');
  fs.rmSync(profile, { recursive: true, force: true });
  fs.mkdirSync(profile, { recursive: true });
  const htmlPath = path.join(testRoot, 'test.html');
  fs.writeFileSync(htmlPath, `<!doctype html><style>
    :root { --color-blob-accent: rgb(12, 34, 56); --color-system-accent-125: rgb(55, 122, 255); }
    .viewCalendar .day.today { color: var(--color-system-accent-125); font-weight: 500; }
    .viewCalendar .day.today .bullet { background: var(--color-system-accent-125); }
    </style><style>${css}</style>
    <div class="widget"><div class="innerWrap viewCalendar"><div class="body"><div class="table"><div class="tableBody">
    <div class="day today" id="day-fixture-20-11-2026"><div class="inner" id="today-inner">20<div class="bullet" id="today-bullet"></div></div></div>
    <div class="day"><div class="inner" id="other-inner">21</div></div>
    </div></div></div></div></div><output id="result"></output><script>
    const result = {
      today: getComputedStyle(document.getElementById('today-inner')).color,
      bullet: getComputedStyle(document.getElementById('today-bullet')).backgroundColor,
      other: getComputedStyle(document.getElementById('other-inner')).color
    };
    document.getElementById('result').textContent = JSON.stringify(result);</script>`);
  const html = execFileSync(chromePath, ['--headless=new', '--no-sandbox', '--no-first-run', '--no-default-browser-check', '--disable-gpu', '--disable-gpu-sandbox', `--user-data-dir=${profile}`, '--dump-dom', `file:///${htmlPath.replace(/\\/g, '/')}`], {
    encoding: 'utf8', timeout: 60000, env: { ...process.env, TEMP: testRoot, TMP: testRoot },
  });
  const result = JSON.parse(html.match(/<output id="result">([^<]*)<\/output>/)[1]);
  assert.equal(result.today, 'rgb(12, 34, 56)', 'today number should compute to the configured accent');
  assert.equal(result.bullet, 'rgb(12, 34, 56)', 'today event dot should compute to the configured accent');
  assert.notEqual(result.other, 'rgb(12, 34, 56)', 'regular days should keep their own color');
});
