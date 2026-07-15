<script lang="ts">
  import { onMount } from 'svelte';
  import { EditorView, type AudioInfo, type WasmColormapName } from '@phonix/ui';
  import { WasmCoreClient } from '$lib/core/WasmCoreClient';
  import { WebAudioPlayback } from '$lib/playback/WebAudioPlayback';

  let client = $state<WasmCoreClient | null>(null);
  let playback = $state<WebAudioPlayback | null>(null);
  let audio = $state<AudioInfo | null>(null);
  let annotationId = $state<bigint | null>(null);
  let cursorTime = $state(0);
  let isPlaying = $state(false);
  let theme = $state<'light' | 'dark'>('light');
  let colormap = $state<WasmColormapName>('Viridis');
  let error = $state('');
  let frame = 0;

  onMount(() => {
    client = new WasmCoreClient();
    playback = new WebAudioPlayback();
    const saved = localStorage.getItem('phonix-theme');
    theme = saved === 'dark' || saved === 'light'
      ? saved
      : window.matchMedia('(prefers-color-scheme: dark)').matches
        ? 'dark'
        : 'light';
    applyTheme(theme);
    const tick = () => {
      if (playback) {
        cursorTime = playback.position;
        isPlaying = playback.playing;
      }
      frame = requestAnimationFrame(tick);
    };
    frame = requestAnimationFrame(tick);
    return () => {
      cancelAnimationFrame(frame);
      client?.destroy();
      playback?.close();
    };
  });

  function applyTheme(next: 'light' | 'dark') {
    document.documentElement.classList.toggle('dark', next === 'dark');
    localStorage.setItem('phonix-theme', next);
  }

  async function handleFile(file: File) {
    if (!client || !playback) return;
    error = '';
    try {
      await playback.load(file);
      const info = await client.importAudio(file);
      audio = info;
      annotationId = await client.createAnnotation(info.id, 0, info.duration);
      cursorTime = 0;
      playback.seek(0);
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    }
  }

  async function handlePlayToggle() {
    if (!playback || !audio) return;
    error = '';
    try {
      isPlaying = await playback.toggle(cursorTime);
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    }
  }

  function handleThemeChange(next: 'light' | 'dark') {
    theme = next;
    applyTheme(next);
  }

  function handleCursorChange(time: number) {
    cursorTime = time;
    playback?.seek(time);
  }
</script>

<EditorView
  {client}
  {audio}
  {annotationId}
  {cursorTime}
  {isPlaying}
  {theme}
  {colormap}
  onFile={handleFile}
  onPlayToggle={handlePlayToggle}
  onThemeChange={handleThemeChange}
  onColormapChange={(next) => (colormap = next)}
  onCursorChange={handleCursorChange}
  onAnnotationChange={(id) => (annotationId = id)}
/>

{#if error}
  <div class="error" role="alert" data-testid="error">{error}</div>
{/if}

<style>
  .error {
    position: fixed;
    right: 1rem;
    bottom: 1rem;
    max-width: min(30rem, calc(100vw - 2rem));
    padding: 0.75rem 0.9rem;
    border: 1px solid color-mix(in oklab, var(--warn), transparent 30%);
    border-radius: 6px;
    background: var(--panel);
    color: var(--warn);
    box-shadow: 0 12px 32px rgba(15, 23, 42, 0.16);
  }
</style>
