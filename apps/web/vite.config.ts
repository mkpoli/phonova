import tailwindcss from '@tailwindcss/vite';
import { sveltekit } from '@sveltejs/kit/vite';
import Icons from 'unplugin-icons/vite';
import { defineConfig } from 'vite';

export default defineConfig({
  plugins: [tailwindcss(), Icons({ compiler: 'svelte' }), sveltekit()],
  worker: {
    format: 'es'
  }
});
