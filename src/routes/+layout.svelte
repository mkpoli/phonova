<script lang="ts">
  import Header from '$lib/components/Header.svelte';
  import Footer from '$lib/components/Footer.svelte';
  import '../app.css';
  import '@fontsource-variable/orbitron';
  let { children } = $props();
</script>

<Header />

{@render children()}

<Footer />
