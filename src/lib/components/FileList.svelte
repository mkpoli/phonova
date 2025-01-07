<script lang="ts">
  import { openFiles, type File } from '../utils/file';

  let { currentFile = $bindable(null), files = $bindable([]) } = $props<{
    currentFile: string | null;
    files: File[];
  }>();
  $inspect('files', files);
  $inspect('currentFile', currentFile);
  $effect(() => {
    // When adding a new file and never set the current file, set it to the first file
    if (!currentFile) {
      currentFile = files[0]?.id;
    }
  });
</script>

{#snippet fileOpenButton()}
  <button class="button-primary m-4" onclick={async () => files.push(...(await openFiles('audio/*')))}
    >Open a file</button
  >
{/snippet}

<section class="bg-gray-200 dark:bg-gray-800 grid grid-rows-[auto_1fr] w-64">
  <h2 class="bg-gray-300 dark:bg-black p-2">Audios</h2>

  <div class="h-full">
    {#if files.length === 0}
      <div class="flex flex-col items-center justify-start h-full gap-2 p-4">
        <p>To start, add an audio file, you can drag and drop it here or</p>
        {@render fileOpenButton()}
        <button class="button-primary" disabled>Record audio</button>
      </div>
    {:else}
      <ul class="w-full">
        {#each files as file}
          <li title={file.id} class="w-full">
            <button class="p-2 w-full" class:selected={currentFile === file.id} onclick={() => (currentFile = file.id)}
              >{file.name}</button
            >
          </li>
        {/each}
      </ul>
      {@render fileOpenButton()}
    {/if}
  </div>
</section>

<style lang="postcss">
  .selected {
    @apply bg-blue-500/50 text-white;
  }
</style>
