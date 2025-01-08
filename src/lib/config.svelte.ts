import { browser } from '$app/environment';

function parseTheme(theme: string): 'dark' | 'light' | 'auto' {
  if (theme === 'dark') return 'dark';
  if (theme === 'light') return 'light';
  return 'auto';
}

export type Theme = 'dark' | 'light' | 'auto';

export const config = (() => {
  let theme: Theme = $state('auto');

  function setTheme(t: Theme) {
    console.log({
      theme,
      browser,
      localStorage: localStorage.getItem('theme'),
      window: window.matchMedia('(prefers-color-scheme: dark)').matches,
    });
    theme = t;
    document.documentElement.classList.toggle('dark', theme == 'dark');
    localStorage.setItem('theme', theme);
  }

  setTheme(
    browser
      ? 'theme' in localStorage
        ? parseTheme(localStorage.getItem('theme') || 'auto')
        : window.matchMedia('(prefers-color-scheme: dark)').matches
        ? 'dark'
        : 'light'
      : 'auto'
  );

  return {
    get theme() {
      return theme;
    },
    set theme(theme: Theme) {
      console.log('set theme', theme);
      setTheme(theme);
    },
  };
})();
