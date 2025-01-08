<script lang="ts">
  import { page } from '$app/state';
  import MingcuteMusicLine from '~icons/mingcute/music-line';
  import MaterialSymbolsSearchInsightsRounded from '~icons/material-symbols/search-insights-rounded';
  import MaterialSymbolsBarChart4BarsRounded from '~icons/material-symbols/bar-chart-4-bars-rounded';
  let currentMode = $derived(
    page.url.pathname == '/analyze' ? 'Analyse' : page.url.pathname == '/plot' ? 'Plot' : 'Audio'
  );
</script>

<footer class="w-full bg-slate-300 p-0">
  <nav class="flex items-center justify-center mx-auto font-orbitron text-xl gap-4">
    <a href="/" class:active={currentMode == 'Audio'}>
      <MingcuteMusicLine class="w-8 h-8" />
      <span> Audio </span>
    </a>
    <a href="/analyze" class:active={currentMode == 'Analyse'}>
      <MaterialSymbolsSearchInsightsRounded class="w-8 h-8" />
      <span> Analyse </span>
    </a>
    <a href="/plot" class:active={currentMode == 'Plot'}>
      <MaterialSymbolsBarChart4BarsRounded class="w-8 h-8" />
      <span> Plot </span>
    </a>
  </nav>
</footer>

<style lang="postcss">
  a {
    @apply flex flex-col items-center border-b-2 border-transparent;
  }

  a.active {
    @apply border-b-2 border-blue-600;
  }
</style>
