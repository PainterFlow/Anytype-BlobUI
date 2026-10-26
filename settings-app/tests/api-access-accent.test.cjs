const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const workspace = path.resolve(__dirname, '../..');
const chromePath = process.env.CHROME_PATH
  || ['C:/Program Files/Google/Chrome/Application/chrome.exe', 'C:/Program Files (x86)/Google/Chrome/Application/chrome.exe']
    .find(candidate => fs.existsSync(candidate));

test('API key access read-only label computes to the configured accent while hovered', {
  skip: !chromePath && 'Google Chrome is required; set CHROME_PATH if it is not installed in the default location',
}, () => {
  const cssPath = process.env.BLOBUI_CSS_PATH || path.join(workspace, 'Dev', '30-icons.css');
  const css = fs.readFileSync(cssPath, 'utf8').replace(/:hover\b/g, '.is-hovered').replace(/<\/style/gi, '<\\/style');
  const testRoot = path.join(workspace, '.test-run', 'api-access-accent');
  fs.mkdirSync(testRoot, { recursive: true });
  const profile = path.join(testRoot, 'chrome-profile');
  fs.rmSync(profile, { recursive: true, force: true });
  fs.mkdirSync(profile, { recursive: true });
  const htmlPath = path.join(testRoot, 'test.html');
  fs.writeFileSync(htmlPath, `<!doctype html><style>:root { --color-blob-accent: rgb(12, 34, 56); }</style><style>${css}</style><button class="apiKeyAccess is-hovered"><span>Read only</span><span class="keyMeta">All spaces</span></button><output id="result"></output><script>document.querySelector('#result').textContent = getComputedStyle(document.querySelector('.apiKeyAccess > span:first-child')).color;</script>`);
  const html = execFileSync(chromePath, ['--headless=new', '--no-sandbox', '--no-first-run', '--no-default-browser-check', '--disable-gpu', '--disable-gpu-sandbox', `--user-data-dir=${profile}`, '--dump-dom', `file:///${htmlPath.replace(/\\/g, '/')}`], {
    encoding: 'utf8', timeout: 30000, env: { ...process.env, TEMP: testRoot, TMP: testRoot },
  });
  const color = html.match(/<output id="result">([^<]*)<\/output>/)?.[1];
  assert.equal(color, 'rgb(12, 34, 56)', 'hovered Read only label should compute to the configured accent');
});
