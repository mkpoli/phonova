<script lang="ts">
  import { onMount } from 'svelte';

  let { phrases }: { phrases: string[] } = $props();

  let index = $state(0);

  onMount(() => {
    if (window.matchMedia('(prefers-reduced-motion: reduce)').matches) return;
    const id = setInterval(() => {
      index = (index + 1) % phrases.length;
    }, 3200);
    return () => clearInterval(id);
  });
</script>

<span class="rolling">
  {#each phrases as phrase, i (phrase)}
    <span class="word" class:active={i === index} aria-hidden={i === index ? undefined : true}
      >{phrase}</span
    >
  {/each}
</span>

<style>
  .rolling {
    display: inline-grid;
    vertical-align: bottom;
  }

  .word {
    grid-area: 1 / 1;
    white-space: nowrap;
    opacity: 0;
    transition: opacity 0.45s ease-in-out;
  }

  .word.active {
    opacity: 1;
  }

  @media (prefers-reduced-motion: reduce) {
    .word {
      transition: none;
    }
  }
</style>
