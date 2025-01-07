<script lang="ts">
  import { openFiles } from '../utils/file';

  let files: { name: string; content: ArrayBuffer }[] = $state([]);
  $inspect('files', files);
</script>

{#snippet fileOpenButton()}
  <button class="button-primary" onclick={async () => files.push(...(await openFiles('audio/*')))}>Open a file</button>
{/snippet}

<section class="bg-gray-200 dark:bg-gray-800 grid grid-rows-[auto_1fr] w-64">
  <h2 class="bg-gray-300 dark:bg-black p-2">Audios</h2>

  <div class="p-2 h-full">
    {#if files.length === 0}
      <div class="flex flex-col items-center justify-start h-full gap-2">
        <p>To start, add an audio file, you can drag and drop it here or</p>
        {@render fileOpenButton()}
        <button class="button-primary" disabled>Record audio</button>
      </div>
    {:else}
      <ul>
        {#each files as file}
          <li>{file.name}</li>
        {/each}
      </ul>
      {@render fileOpenButton()}
    {/if}
  </div>
</section>
