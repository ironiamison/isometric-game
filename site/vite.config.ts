import { sveltekit } from '@sveltejs/kit/vite';
import tailwindcss from '@tailwindcss/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [tailwindcss(), sveltekit()],
  server: {
    port: 5173,
    strictPort: true,
    proxy: {
      '/api': 'http://localhost:2567',
      '/health': 'http://localhost:2567',
      '/matchmake': 'http://localhost:2567',
      '/spectate': {
        target: 'ws://localhost:2567',
        ws: true,
      },
      '/ws': {
        target: 'ws://localhost:2567',
        ws: true,
      },
    },
  },
});
