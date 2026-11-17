import { cp, mkdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

const projectDirectory = path.dirname(fileURLToPath(import.meta.url));
const uiDirectory = path.join(projectDirectory, 'ui');
const markdownDirectory = path.join(projectDirectory, 'markdowns');
const markdownAssets = ['Changelogs.md', 'dev-guide.md', 'anyblob-guide.md', 'md-images.yaml'];

async function readMarkdownAsset(name) {
  const source = path.join(markdownDirectory, name);
  try {
    return await readFile(source);
  } catch (error) {
    if (error?.code === 'ENOENT') {
      throw new Error(`Required Settings Markdown asset is missing: ${source}`);
    }
    throw error;
  }
}

export default defineConfig({
  root: uiDirectory,
  publicDir: false,
  build: {
    outDir: path.join(projectDirectory, 'frontend-dist'),
    emptyOutDir: true,
    assetsDir: 'assets',
    commonjsOptions: { include: [/node_modules/, /marked\.umd\.js$/] },
    rollupOptions: {
      output: {
        entryFileNames: 'assets/[name]-[hash].js',
        chunkFileNames: 'assets/[name]-[hash].js',
        assetFileNames: 'assets/[name]-[hash][extname]',
        manualChunks(id) {
          if (id.includes('/node_modules/')) return 'react-runtime';
          if (id.endsWith('/vendor/marked.umd.js')) return 'markdown-parser';
        },
      }
    }
  },
  plugins: [
    {
      name: 'copy-settings-static-assets',
      async buildStart() {
        for (const name of markdownAssets) await readMarkdownAsset(name);
      },
      configureServer(server) {
        server.middlewares.use('/dev-guide.md', async (_request, response, next) => {
          try {
            const guide = await readMarkdownAsset('dev-guide.md');
            response.setHeader('Content-Type', 'text/markdown; charset=utf-8');
            response.end(guide);
          } catch (error) {
            next(error);
          }
        });
        server.middlewares.use('/anyblob-guide.md', async (_request, response, next) => {
          try {
            const guide = await readMarkdownAsset('anyblob-guide.md');
            response.setHeader('Content-Type', 'text/markdown; charset=utf-8');
            response.end(guide);
          } catch (error) {
            next(error);
          }
        });
        server.middlewares.use('/md-images.yaml', async (_request, response, next) => {
          try {
            const manifest = await readMarkdownAsset('md-images.yaml');
            response.setHeader('Content-Type', 'text/yaml; charset=utf-8');
            response.end(manifest);
          } catch (error) {
            next(error);
          }
        });
      },
      async closeBundle() {
        const outputDirectory = path.join(projectDirectory, 'frontend-dist');
        await mkdir(outputDirectory, { recursive: true });
        for (const directory of ['cdn']) {
          await cp(path.join(uiDirectory, directory), path.join(outputDirectory, directory), {
            recursive: true,
            force: true
          });
        }
        for (const name of markdownAssets) {
          await cp(path.join(markdownDirectory, name), path.join(outputDirectory, name));
        }
        await cp(path.join(uiDirectory, 'vendor', 'MARKED-LICENSE.txt'), path.join(outputDirectory, 'MARKED-LICENSE.txt'));
      }
    }
  ],
  test: {
    include: ['**/*.test.{js,jsx}'],
    environment: 'jsdom',
    globals: true,
    clearMocks: true
  }
});
