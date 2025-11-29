import { defineConfig } from 'vite'
import wasm from 'vite-plugin-wasm'

export default defineConfig({
  plugins: [
    wasm(),
  ],
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
    target: 'esnext',
    sourcemap: true,
    chunkSizeWarningLimit: 4000,
  },
})
