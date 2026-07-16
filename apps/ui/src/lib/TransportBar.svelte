<script lang="ts">
  import IconPlay from '~icons/lucide/play';
  import IconPause from '~icons/lucide/pause';
  import IconSun from '~icons/lucide/sun';
  import IconMoon from '~icons/lucide/moon';
  import IconFileAudio from '~icons/lucide/file-audio-2';
  import { formatTime, type AudioInfo, type WasmColormapName } from './types';

  interface Props {
    audio: AudioInfo | null;
    cursorTime: number;
    isPlaying: boolean;
    theme: 'light' | 'dark';
    colormap: WasmColormapName;
    onFile: (file: File) => void;
    onPlayToggle: () => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
    onColormapChange: (colormap: WasmColormapName) => void;
  }

  let {
    audio,
    cursorTime,
    isPlaying,
    theme,
    colormap,
    onFile,
    onPlayToggle,
    onThemeChange,
    onColormapChange
  }: Props = $props();

  let dragging = $state(false);

  function takeFileList(files: FileList | null) {
    const file = files?.item(0);
    if (file) onFile(file);
  }

  function handleDrop(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    takeFileList(event.dataTransfer?.files ?? null);
  }
</script>

<div
  class:dragging
  class="transport"
  role="group"
  aria-label="Transport"
  data-testid="transport"
  ondragover={(event) => {
    event.preventDefault();
    dragging = true;
  }}
  ondragleave={() => (dragging = false)}
  ondrop={handleDrop}
>
  <button
    class="icon-button play"
    type="button"
    aria-label={isPlaying ? 'Pause' : 'Play'}
    onclick={onPlayToggle}
    disabled={!audio}
  >
    {#if isPlaying}
      <IconPause aria-hidden="true" />
    {:else}
      <IconPlay aria-hidden="true" />
    {/if}
  </button>

  <label class="drop-target">
    <IconFileAudio class="drop-icon" aria-hidden="true" />
    <span>{audio?.name ?? 'Drop or choose a WAV'}</span>
    <input
      data-testid="file-input"
      type="file"
      accept=".wav,audio/wav,audio/x-wav"
      onchange={(event) => takeFileList(event.currentTarget.files)}
    />
  </label>

  <div class="readout" data-testid="cursor-readout" data-cursor-time={cursorTime.toFixed(6)}>
    {formatTime(cursorTime)}
  </div>

  <select
    class="palette-select"
    aria-label="Spectrogram palette"
    value={colormap}
    onchange={(event) => onColormapChange(event.currentTarget.value as WasmColormapName)}
  >
    <option value="Viridis">Viridis</option>
    <option value="Magma">Magma</option>
    <option value="Grayscale">Gray</option>
  </select>

  <button
    class="icon-button theme"
    type="button"
    aria-label="Toggle theme"
    title={theme === 'light' ? 'Switch to dark' : 'Switch to light'}
    onclick={() => onThemeChange(theme === 'light' ? 'dark' : 'light')}
  >
    {#if theme === 'light'}
      <IconMoon aria-hidden="true" />
    {:else}
      <IconSun aria-hidden="true" />
    {/if}
  </button>
</div>

<style>
  .transport {
    display: grid;
    grid-template-columns: auto minmax(12rem, 1fr) auto auto auto;
    gap: 0.5rem;
    align-items: center;
    min-height: 3rem;
    padding: 0.5rem 0.85rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel);
  }

  .transport.dragging {
    outline: 2px solid var(--accent);
    outline-offset: -3px;
  }

  .icon-button,
  .palette-select,
  .drop-target {
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-md);
    background: var(--panel-soft);
    color: var(--text);
    min-height: 2.1rem;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      box-shadow var(--t-fast);
  }

  .icon-button {
    width: 2.4rem;
    display: grid;
    place-items: center;
    font-size: 1.05rem;
  }

  .icon-button:hover:not(:disabled) {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 40%, var(--chrome-strong));
  }

  .play {
    color: var(--accent-strong);
    border-color: color-mix(in oklab, var(--accent) 45%, var(--chrome-strong));
    background: var(--accent-tint);
  }

  button:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }

  .drop-target {
    display: flex;
    align-items: center;
    gap: 0.5rem;
    padding: 0 0.8rem;
    overflow: hidden;
    color: var(--muted);
  }

  .drop-target:hover {
    background: var(--panel);
  }

  .drop-target :global(.drop-icon) {
    flex: none;
    font-size: 1.05rem;
    color: var(--muted);
  }

  .drop-target span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text);
  }

  .drop-target input {
    display: none;
  }

  .readout {
    min-width: 5.75rem;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
    text-align: right;
    font-size: 0.95rem;
  }

  .palette-select {
    padding: 0 0.65rem;
  }

  @media (max-width: 720px) {
    .transport {
      grid-template-columns: auto 1fr auto;
    }

    .palette-select {
      display: none;
    }
  }
</style>
