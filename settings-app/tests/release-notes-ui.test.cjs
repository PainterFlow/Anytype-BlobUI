const assert = require('node:assert/strict');
const { spawnSync } = require('node:child_process');
const path = require('node:path');
const test = require('node:test');

const harness = path.join(__dirname, 'ui-state.test.cjs');

for (const scenario of ['escape', 'backdrop', 'history', 'offline', 'parse-error']) {
  test(`release notes ${scenario} lifecycle`, () => {
    const result = spawnSync(process.execPath, [harness, `--release-notes-scenario=${scenario}`], {
      encoding: 'utf8',
      timeout: 30000,
      windowsHide: true,
    });
    assert.equal(result.error, undefined, result.error && result.error.message);
    assert.equal(result.status, 0, `${result.stdout}\n${result.stderr}`);
    assert.match(result.stdout, scenario === 'parse-error'
      ? /parse failures stay pending/
      : new RegExp(`Release notes ${scenario} dismissal lifecycle passed`));
  });
}
