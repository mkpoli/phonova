import forms from '@tailwindcss/forms';
import type { Config } from 'tailwindcss';

export default {
  content: ['./src/**/*.{html,js,svelte,ts}'],

  darkMode: 'selector',

  theme: {
    extend: {},
    fontFamily: {
      orbitron: ['Orbitron Variable', 'sans-serif'],
    },
  },

  plugins: [forms],
} satisfies Config;
