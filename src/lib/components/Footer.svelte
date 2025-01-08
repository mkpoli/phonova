<script lang="ts">
  import { page } from '$app/state';
  let currentMode = $derived(
    page.url.pathname == '/analyze' ? 'Analyse' : page.url.pathname == '/plot' ? 'Plot' : 'Audio'
  );
</script>

<footer class="w-full p-4">
  {currentMode}
  <nav class="flex items-center justify-center mx-auto font-orbitron text-xl gap-4">
    <a href="/">Audio</a>
    <a href="/analyze">Analyse</a>
    <a href="/plot">Plot</a>
  </nav>
</footer>
