<script lang="ts">
  import { page } from '$app/state';
  let currentMode = $derived(
    page.url.pathname == '/analyze' ? 'Analyse' : page.url.pathname == '/plot' ? 'Plot' : 'Audio'
  );
</script>

<footer class="w-full bg-slate-300 p-0">
  <nav class="flex items-center justify-center mx-auto font-orbitron text-xl gap-4">
    <a href="/" class:active={currentMode == 'Audio'}>Audio</a>
    <a href="/analyze" class:active={currentMode == 'Analyse'}>Analyse</a>
    <a href="/plot" class:active={currentMode == 'Plot'}>Plot</a>
  </nav>
</footer>

<style lang="postcss">
  a {
    @apply flex border-b-2 border-transparent;
  }

  a.active {
    @apply border-b-2 border-blue-600;
  }
</style>
