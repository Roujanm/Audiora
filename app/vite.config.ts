import { defineConfig } from 'vite';
import { svelte } from '@sveltejs/vite-plugin-svelte';

// The engine only accepts WebSocket origins on its allow-list, which includes
// http://localhost:5173, so the dev server port is fixed.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: { host: 'localhost', port: 5173, strictPort: true },
  build: { target: 'es2022', sourcemap: false },
});
