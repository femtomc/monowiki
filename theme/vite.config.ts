import { defineConfig } from 'vite';
import { resolve } from 'path';

export default defineConfig({
  build: {
    lib: {
      entry: resolve(__dirname, 'src/main.ts'),
      name: 'MonowikiTheme',
      fileName: 'bundle',
      formats: ['es'],
    },
    rollupOptions: {
      output: {
        assetFileNames: 'bundle.[ext]',
      },
    },
    outDir: 'dist',
    emptyOutDir: true,
    minify: 'terser',
    sourcemap: true,
  },
  test: {
    globals: true,
    environment: 'jsdom',
  },
});
