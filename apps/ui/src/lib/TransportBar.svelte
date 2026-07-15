<script lang="ts">
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
  <button class="icon-button" type="button" aria-label={isPlaying ? 'Pause' : 'Play'} onclick={onPlayToggle} disabled={!audio}>
    {#if isPlaying}
      <span aria-hidden="true">||</span>
    {:else}
      <span class="play-icon" aria-hidden="true"></span>
    {/if}
  </button>

  <label class="drop-target">
    <span>{audio?.name ?? 'Drop WAV'}</span>
    <input data-testid="file-input" type="file" accept=".wav,audio/wav,audio/x-wav" onchange={(event) => takeFileList(event.currentTarget.files)} />
  </label>

  <div class="readout" data-testid="cursor-readout" data-cursor-time={cursorTime.toFixed(6)}>
    {formatTime(cursorTime)}
  </div>

  <select aria-label="Spectrogram palette" value={colormap} onchange={(event) => onColormapChange(event.currentTarget.value as WasmColormapName)}>
    <option value="Viridis">Viridis</option>
    <option value="Magma">Magma</option>
    <option value="Grayscale">Gray</option>
  </select>

  <button class="theme-button" type="button" aria-label="Toggle theme" onclick={() => onThemeChange(theme === 'light' ? 'dark' : 'light')}>
    {theme === 'light' ? 'Dark' : 'Light'}
  </button>
</div>

<style>
  .transport {
    display: grid;
    grid-template-columns: auto minmax(12rem, 1fr) auto auto auto;
    gap: 0.5rem;
    align-items: center;
    min-height: 3rem;
    padding: 0.5rem 0.75rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel);
  }

  .transport.dragging {
    outline: 2px solid var(--accent);
    outline-offset: -3px;
  }

  .icon-button,
  .theme-button,
  select,
  .drop-target {
    border: 1px solid var(--chrome-strong);
    border-radius: 6px;
    background: var(--panel-soft);
    color: var(--text);
    min-height: 2rem;
  }

  .icon-button {
    width: 2.25rem;
    display: grid;
    place-items: center;
  }

  .play-icon {
    width: 0;
    height: 0;
    border-top: 0.34rem solid transparent;
    border-bottom: 0.34rem solid transparent;
    border-left: 0.52rem solid currentColor;
    transform: translateX(0.08rem);
  }

  button:disabled {
    opacity: 0.45;
  }

  .drop-target {
    display: flex;
    align-items: center;
    padding: 0 0.75rem;
    overflow: hidden;
  }

  .drop-target span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .drop-target input {
    display: none;
  }

  .readout {
    min-width: 5.5rem;
    font-variant-numeric: tabular-nums;
    color: var(--muted);
    text-align: right;
  }

  select,
  .theme-button {
    padding: 0 0.6rem;
  }

  @media (max-width: 720px) {
    .transport {
      grid-template-columns: auto 1fr auto;
    }

    select,
    .theme-button {
      display: none;
    }
  }
</style>
