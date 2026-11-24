const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('path');
const { execFileSync } = require('node:child_process');

const workspace = path.resolve(__dirname, '../..');
const chromePath = process.env.CHROME_PATH
  || ['C:/Program Files/Google/Chrome/Application/chrome.exe', 'C:/Program Files (x86)/Google/Chrome/Application/chrome.exe']
    .find(candidate => fs.existsSync(candidate));

test('the last native accent row in a boxed settings list draws no extra bottom border', {
  skip: !chromePath && 'Google Chrome is required; set CHROME_PATH if it is not installed in the default location',
}, () => {
  const css = fs.readFileSync(path.join(workspace, 'settings-app', 'ui', 'styles.css'), 'utf8').replace(/<\/style/gi, '<\\/style');
  const testRoot = path.join(workspace, '.test-run', 'settings-row-border');
  fs.mkdirSync(testRoot, { recursive: true });
  const profile = path.join(testRoot, 'chrome-profile');
  fs.rmSync(profile, { recursive: true, force: true });
  fs.mkdirSync(profile, { recursive: true });
  const htmlPath = path.join(testRoot, 'test.html');
  fs.writeFileSync(htmlPath, `<!doctype html><style>${css}</style>
    <div class="settings-list">
      <label class="native-accent-control settings-row" id="first"><span>First</span></label>
      <label class="native-accent-control settings-row" id="last"><span>Last</span></label>
    </div>
    <div class="settings-list">
      <label class="native-accent-control settings-row" id="single"><span>Single</span></label>
    </div><output id="result"></output><script>
    const width = id => getComputedStyle(document.getElementById(id)).borderBottomWidth;
    document.getElementById('result').textContent = JSON.stringify({ first: width('first'), last: width('last'), single: width('single') });</script>`);
  const html = execFileSync(chromePath, ['--headless=new', '--no-sandbox', '--no-first-run', '--no-default-browser-check', '--disable-gpu', '--disable-gpu-sandbox', `--user-data-dir=${profile}`, '--dump-dom', `file:///${htmlPath.replace(/\\/g, '/')}`], {
    encoding: 'utf8', timeout: 60000, env: { ...process.env, TEMP: testRoot, TMP: testRoot },
  });
  const result = JSON.parse(html.match(/<output id="result">([^<]*)<\/output>/)[1]);
  assert.equal(result.first, '1px', 'separating rows keep their bottom border');
  assert.equal(result.last, '0px', 'the last row must not double the list border');
  assert.equal(result.single, '0px', 'a single-row list must not double the list border');
});
