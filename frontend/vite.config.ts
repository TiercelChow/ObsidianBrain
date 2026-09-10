import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  plugins: [vue()],
  server: {
    port: 5173,
    proxy: {
      '/v1': {
        target: 'http://127.0.0.1:9876',
        changeOrigin: true,
      },
    },
  },
  resolve: {
    alias: {
      '@': '/src',
      // Vite otherwise selects DOM-based browser entries while bundling the
      // Markdown worker. Workers expose neither DOMParser nor document, so use
      // the packages' equivalent pure-JavaScript worker implementations.
      'hast-util-from-html-isomorphic': fileURLToPath(
        new URL(
          './node_modules/hast-util-from-html-isomorphic/lib/index.js',
          import.meta.url,
        ),
      ),
      'decode-named-character-reference': fileURLToPath(
        new URL(
          './node_modules/decode-named-character-reference/index.js',
          import.meta.url,
        ),
      ),
    },
  },
})
