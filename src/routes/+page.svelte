<script lang="ts">
  import { invoke } from '@tauri-apps/api/core';
  import FileList from '$lib/components/FileList.svelte';
  import { type File } from '$lib/utils/file';
  let name = $state('');
  let greetMsg = $state('');
  import formatDuration from 'format-duration';

  async function greet(event: Event) {
    event.preventDefault();
    // Learn more about Tauri commands at https://tauri.app/develop/calling-rust/
    greetMsg = await invoke('greet', { name });
  }

  let files: File[] = $state([]);
  let fileMap: Record<string, File> = $derived(Object.fromEntries(files.map((file: File) => [file.id, file])));

  // UUID of the current file
  let currentFileUUID: string | null = $state(null);
  let currentFile: File | null = $derived(currentFileUUID ? fileMap[currentFileUUID] : null);

  interface AudioInfo {
    sample_rate: number;
    channels: number;
    bits_per_sample: number;
    length: number;
    duration: number;
    rms: number;
  }

  let parsedFiles: Record<string, Promise<AudioInfo>> = $derived(
    Object.fromEntries(
      files.map(
        ({ id, name, content }: File) => [id, invoke('read_wav', { buffer: content })] as [string, Promise<AudioInfo>]
      )
    )
  );

  //  Record<string, Promise<{ name: string; audio: string }>>

  $inspect('parsedFiles', parsedFiles);
</script>

<main class="container">
  <FileList bind:files bind:currentFile={currentFileUUID} />
  <div>
    <h1 class="text-4xl font-orbitron">Welcome to Phonova</h1>

    {#if currentFileUUID && fileMap[currentFileUUID]}
      <p>{fileMap[currentFileUUID].id}</p>
      <p>{fileMap[currentFileUUID].name}</p>
      <p>{currentFile?.content.byteLength}</p>
      {#await parsedFiles[currentFileUUID]}
        <p>Loading...</p>
      {:then parsedFile}
        <p>parsedFile.sample_rate: {parsedFile.sample_rate}</p>
        <p>parsedFile.channels: {parsedFile.channels}</p>
        <p>parsedFile.bits_per_sample: {parsedFile.bits_per_sample}</p>
        <p>parsedFile.length: {parsedFile.length}</p>
        <p>parsedFile.duration: {formatDuration(parsedFile.duration * 1000)}</p>
        <p>parsedFile.rms: {parsedFile.rms}</p>
      {/await}
    {/if}

    <form class="row" onsubmit={greet}>
      <input id="greet-input" placeholder="Enter a name..." bind:value={name} />
      <button type="submit" class="button-primary">Greet</button>
    </form>
    <p>{greetMsg}</p>
  </div>
</main>

<style lang="postcss">
  .logo.vite:hover {
    filter: drop-shadow(0 0 2em #747bff);
  }

  .logo.svelte-kit:hover {
    filter: drop-shadow(0 0 2em #ff3e00);
  }

  :root {
    font-family: Inter, Avenir, Helvetica, Arial, sans-serif;
    font-size: 16px;
    line-height: 24px;
    font-weight: 400;

    color: #0f0f0f;
    background-color: #f6f6f6;

    font-synthesis: none;
    text-rendering: optimizeLegibility;
    -webkit-font-smoothing: antialiased;
    -moz-osx-font-smoothing: grayscale;
    -webkit-text-size-adjust: 100%;
  }

  .container {
    margin: 0;
    padding-top: 10vh;
    display: flex;
    justify-content: center;
    text-align: center;
  }

  .logo {
    height: 6em;
    padding: 1.5em;
    will-change: filter;
    transition: 0.75s;
  }

  .logo.tauri:hover {
    filter: drop-shadow(0 0 2em #24c8db);
  }

  .row {
    display: flex;
    justify-content: center;
  }

  a {
    font-weight: 500;
    color: #646cff;
    text-decoration: inherit;
  }

  a:hover {
    color: #535bf2;
  }

  h1 {
    text-align: center;
  }

  input {
    font-size: 1em;
    font-weight: 500;
    font-family: inherit;
    transition: border-color 0.25s;
    box-shadow: 0 2px 2px rgba(0, 0, 0, 0.2);
    @apply text-gray-900 dark:text-gray-200 bg-white dark:bg-gray-900;
  }

  #greet-input {
    margin-right: 5px;
  }
</style>
