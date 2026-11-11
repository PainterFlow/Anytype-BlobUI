import { cp, mkdir, readFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { defineConfig } from 'vitest/config';

const projectDirectory = path.dirname(fileURLToPath(import.meta.url));
const uiDirectory = path.join(projectDirectory, 'ui');

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
      configureServer(server) {
        server.middlewares.use('/dev-guide.md', async (_request, response, next) => {
          try {
            const guide = await readFile(path.resolve(projectDirectory, '..', 'dev-guide.md'));
            response.setHeader('Content-Type', 'text/markdown; charset=utf-8');
            response.end(guide);
          } catch (error) {
            next(error);
          }
        });
        server.middlewares.use('/md-images.yaml', async (_request, response, next) => {
          try {
            const manifest = await readFile(path.resolve(projectDirectory, '..', 'md-images.yaml'));
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
        await cp(path.resolve(projectDirectory, '..', 'dev-guide.md'), path.join(outputDirectory, 'dev-guide.md'));
        await cp(path.resolve(projectDirectory, '..', 'md-images.yaml'), path.join(outputDirectory, 'md-images.yaml'));
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
