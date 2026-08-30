import { fileURLToPath, URL } from 'node:url'
import { defineConfig } from 'vite'
import vue from '@vitejs/plugin-vue'

export default defineConfig({
  base: './',
  plugins: [vue()],
  build: {
    outDir: fileURLToPath(new URL('../module/webroot', import.meta.url)),
    emptyOutDir: true,
    target: 'es2018',
    sourcemap: false,
  },
})
