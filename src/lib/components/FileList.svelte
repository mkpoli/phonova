<script lang="ts">
  const files: { name: string }[] = [];
</script>

<section class="bg-gray-200 dark:bg-gray-800">
  <h2 class="bg-gray-300 dark:bg-black p-2">Audios</h2>

  <div class="p-2 h-full">
    {#if files.length === 0}
      <p>No files found</p>
    {:else}
      <ul>
        {#each files as file}
          <li>{file.name}</li>
        {/each}
      </ul>
    {/if}
  </div>
</section>
