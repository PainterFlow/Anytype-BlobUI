const fs = require('node:fs/promises');
const path = require('node:path');

const output = path.resolve(__dirname, '../frontend-dist');
const origin = 'http://blobui.test';
const contentTypes = new Map([
  ['.html', 'text/html'],
  ['.js', 'application/javascript'],
  ['.css', 'text/css'],
  ['.md', 'text/plain'],
  ['.yaml', 'text/yaml'],
  ['.svg', 'image/svg+xml'],
  ['.png', 'image/png'],
  ['.mp4', 'video/mp4'],
]);

async function loadSettings(page) {
  await fs.access(path.join(output, 'index.html')).catch(() => {
    throw new Error('Build the Settings frontend with npm run build before running the browser tests.');
  });
  await page.route(origin + '/**', async route => {
    const url = new URL(route.request().url());
    const file = path.resolve(output, '.' + decodeURIComponent(url.pathname === '/' ? '/index.html' : url.pathname));
    if (!file.startsWith(output + path.sep)) return route.abort();
    try {
      await route.fulfill({ path: file, contentType: contentTypes.get(path.extname(file)) });
    } catch (error) {
      if (error.code !== 'ENOENT') throw error;
      await route.fulfill({ status: 404, body: 'Fixture asset missing' });
    }
  });
  await page.goto(origin + '/');
}

async function openFormulas(page) {
  await page.locator('#kanban-views-nav').click();
  await page.locator('#kanban-views-section-links').getByRole('button', { name: 'Kanban Calculations', exact: true }).click();
}

module.exports = { loadSettings, openFormulas };
