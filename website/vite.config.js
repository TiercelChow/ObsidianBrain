import { defineConfig } from 'vite'
import { fileURLToPath } from 'node:url'

export default defineConfig({
  base: '/ObsidianBrain/',
  build: {
    target: 'es2020',
    rollupOptions: {
      input: {
        home: fileURLToPath(new URL('./index.html', import.meta.url)),
        manual: fileURLToPath(new URL('./manual/index.html', import.meta.url)),
        llmWiki: fileURLToPath(new URL('./llm-wiki/index.html', import.meta.url)),
      },
    },
  },
})
