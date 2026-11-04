import { cp, mkdir } from 'node:fs/promises';
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
      async closeBundle() {
        const outputDirectory = path.join(projectDirectory, 'frontend-dist');
        await mkdir(outputDirectory, { recursive: true });
        for (const directory of ['docs', 'cdn']) {
          await cp(path.join(uiDirectory, directory), path.join(outputDirectory, directory), {
            recursive: true,
            force: true
          });
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
