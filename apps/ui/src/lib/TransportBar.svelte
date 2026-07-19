<script lang="ts">
  import IconPlay from '~icons/lucide/play';
  import IconPause from '~icons/lucide/pause';
  import IconSun from '~icons/lucide/sun';
  import IconMoon from '~icons/lucide/moon';
  import IconFileAudio from '~icons/lucide/file-audio-2';
  import { formatTime, type AudioInfo } from './types';
  import PalettePicker from './PalettePicker.svelte';
  import type { CustomRamp, PaletteSelection } from './palette';

  interface Props {
    audio: AudioInfo | null;
    cursorTime: number;
    isPlaying: boolean;
    theme: 'light' | 'dark';
    palette: PaletteSelection;
    customRamps: CustomRamp[];
    onFile: (file: File) => void;
    onPlayToggle: () => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
    onPaletteChange: (palette: PaletteSelection) => void;
    onNewRamp: () => void;
    onEditRamp: (ramp: CustomRamp) => void;
  }

  let {
    audio,
    cursorTime,
    isPlaying,
    theme,
    palette,
    customRamps,
    onFile,
    onPlayToggle,
    onThemeChange,
    onPaletteChange,
    onNewRamp,
    onEditRamp
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
    <span>{audio?.name ?? 'Drop or choose a recording'}</span>
    <input
      data-testid="file-input"
      type="file"
      accept=".wav,audio/wav,audio/x-wav,.aiff,.aif,audio/aiff,.flac,audio/flac"
      onchange={(event) => takeFileList(event.currentTarget.files)}
    />
  </label>

  <div class="readout" data-testid="cursor-readout" data-cursor-time={cursorTime.toFixed(6)}>
    {formatTime(cursorTime)}
  </div>

  <div class="palette-slot">
    <PalettePicker
      {palette}
      {customRamps}
      onSelect={onPaletteChange}
      {onNewRamp}
      {onEditRamp}
    />
  </div>

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

  .palette-slot {
    display: inline-flex;
    min-width: 0;
  }

  @media (max-width: 720px) {
    .transport {
      grid-template-columns: auto 1fr auto;
    }

    .palette-slot {
      display: none;
    }
  }
</style>
