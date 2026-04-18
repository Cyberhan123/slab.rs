import { defineConfig } from 'vitest/config';
import react from '@vitejs/plugin-react';
import path from 'path';

export default defineConfig({
  plugins: [react()],
  test: {
    globals: true,
    environment: 'jsdom',
    setupFiles: ['./vitest.setup.ts'],
    css: true,
    exclude: [
      '**/node_modules/**',
      '**/dist/**',
      'tests/e2e/**',
    ],
    coverage: {
      provider: 'v8',
      reporter: ['text', 'json', 'html'],
      exclude: [
        'node_modules/',
        'vitest.setup.ts',
        '**/*.config.*',
        '**/dist/**',
        '**/e2e/**',
      ],
    },
  },
  resolve: {
    dedupe: ['react', 'react-dom'],
    alias: {
      '@': path.resolve(__dirname, './src'),
      '@slab/components': path.resolve(__dirname, '../slab-components/src/index.ts'),
      '@slab/components/*': path.resolve(__dirname, '../slab-components/src/*'),
      '@slab/i18n': path.resolve(__dirname, '../slab-i18n/src/index.ts'),
    },
  },
});
