const test = require('node:test');
const assert = require('node:assert/strict');
const fs = require('node:fs');
const path = require('node:path');
const { execFileSync } = require('node:child_process');

const workspace = path.resolve(__dirname, '../..');
const chrome = process.env.CHROME_PATH || 'C:/Program Files/Google/Chrome/Application/chrome.exe';

test('compiled accents tint search matches and the slash checkbox without repainting its white tick', {
  skip: !fs.existsSync(chrome) && 'Chrome is required',
}, () => {
  const root = path.join(workspace, '.test-run', 'search-checkbox-accent');
  fs.mkdirSync(root, { recursive: true });
  for (const accent of ['native', 'preset', 'custom', 'alpha']) {
    for (const enabled of [false, true]) {
      const css = fs.readFileSync(path.join(workspace, '.test-run', 'selections-compiled', `${accent}-${enabled}.css`), 'utf8');
      const file = path.join(root, `${accent}-${enabled}.html`);
      fs.writeFileSync(file, `<!doctype html><html class="themeDark"><style>
        :root { color-scheme:dark; --color-system-accent-100:rgb(55,122,255); }
        body { background:#171717; color:white; }
        markupsearch { color:#ffcc00; } markupsearch.active { color:#ffcc00; background:#333; }
        .icon.menuBlockTextCheckbox { color:var(--color-system-accent-100); }
        </style><style>${css}</style>
        <markupsearch id="match">Match</markupsearch><markupsearch class="active" id="active">Active match</markupsearch>
        <div class="icon menuBlockTextCheckbox iconMain hasSvg"><svg viewBox="0 0 20 20" style="width:20px;height:20px">
        <path id="circle" d="M2 10a8 8 0 1 0 16 0a8 8 0 1 0-16 0" fill="currentColor"/>
        <path id="tick" d="m6.5 10.5 3 3 4-7" fill="white"/></svg></div>
        <div class="icon unrelated" style="color:rgb(111,111,111)"><svg><path id="unrelated" fill="currentColor"/></svg></div>
        <span id="accent" style="color:var(--color-blob-accent)"></span><output id="result"></output>
        <script>
        const color = id => getComputedStyle(document.getElementById(id)).color;
        const fill = id => getComputedStyle(document.getElementById(id)).fill;
        document.getElementById('result').textContent = JSON.stringify({
          accent:color('accent'), match:color('match'), active:color('active'), circle:fill('circle'), tick:fill('tick'), unrelated:fill('unrelated')
        });</script></html>`);
      const output = execFileSync(chrome, ['--headless=new', '--no-sandbox', '--no-first-run', '--no-default-browser-check', '--disable-gpu', `--user-data-dir=${path.join(root, 'profile')}`, '--dump-dom', `file:///${file.replace(/\\/g, '/')}`], {
        encoding:'utf8', timeout:30000, env:{ ...process.env, TEMP:root, TMP:root }, stdio:['ignore', 'pipe', 'pipe'],
      });
      const result = JSON.parse(output.match(/<output id="result">([^<]+)<\/output>/)[1]);
      assert.equal(result.match, result.accent, `${accent}/${enabled} inactive match`);
      assert.equal(result.active, result.accent, `${accent}/${enabled} active match`);
      assert.equal(result.circle, result.accent, `${accent}/${enabled} checkbox circle`);
      assert.equal(result.tick, 'rgb(255, 255, 255)');
      assert.equal(result.unrelated, 'rgb(111, 111, 111)');
    }
  }
});
