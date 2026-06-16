import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

export default defineConfig({
  base: '/mapper/',
  plugins: [react()],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, './src'),
      '@components': path.resolve(__dirname, './src/components'),
      '@core': path.resolve(__dirname, './src/core'),
      '@state': path.resolve(__dirname, './src/state'),
      '@types': path.resolve(__dirname, './src/types'),
    },
  },
  server: {
    proxy: {
      '/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
      '/mapper/api': {
        target: 'http://localhost:3000',
        changeOrigin: true,
        rewrite: (path) => path.replace(/^\/mapper/, ''),
      },
      // Login/logout must reach the backend so the session cookie is set; without
      // these, logging in through the Vite dev server silently fails and every
      // /mapper/api call returns 401.
      '/mapper/login': { target: 'http://localhost:3000', changeOrigin: true },
      '/mapper/logout': { target: 'http://localhost:3000', changeOrigin: true },
      '/mapper-config.json': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
      '/mapper/mapper-config.json': {
        target: 'http://localhost:3000',
        changeOrigin: true,
      },
    },
  },
})
