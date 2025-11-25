import { defineConfig } from 'vite'

export default defineConfig({
  server: {
    port: 5173,
    // Proxy API requests to the collab daemon during dev
    proxy: {
      '/api': {
        target: 'http://localhost:8787',
        changeOrigin: true,
      },
      '/ws': {
        target: 'ws://localhost:8787',
        ws: true,
      },
    },
  },
  build: {
    outDir: 'dist',
    sourcemap: true,
  },
})
