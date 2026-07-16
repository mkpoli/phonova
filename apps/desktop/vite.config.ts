import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import Icons from 'unplugin-icons/vite';
import { defineConfig } from 'vite';

// Tauri drives this dev server; the port is fixed so `devUrl` in the Tauri
// config resolves, and the terminal is left for the Rust build's own output.
export default defineConfig({
  plugins: [tailwindcss(), Icons({ compiler: 'svelte' }), sveltekit()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true
  }
});
