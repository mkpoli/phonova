<script lang="ts">
  import Footer from '$lib/components/Footer.svelte';
  import '../app.css';
  import '@fontsource-variable/orbitron';
  let { children } = $props();
</script>

{@render children()}

<Footer />
