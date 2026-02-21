import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

const appBase = process.env.WEB_STATS_BASE ?? '/'

export default defineConfig({
  base: appBase,
  plugins: [react(), tailwindcss()],
  server: {
    proxy: {
      '/api': 'http://localhost:2567',
    },
  },
})
