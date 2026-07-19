<script lang="ts">
  import IconSkipBack from '~icons/lucide/skip-back';
  import IconSkipForward from '~icons/lucide/skip-forward';
  import IconPlay from '~icons/lucide/play';
  import IconPause from '~icons/lucide/pause';
  import IconRepeat from '~icons/lucide/repeat';
  import IconMic from '~icons/lucide/mic';
  import IconAudioLines from '~icons/lucide/audio-lines';
  import IconImage from '~icons/lucide/image';
  import IconPanelRight from '~icons/lucide/panel-right';
  import { formatTime, type AudioInfo } from './types';

  interface Props {
    audio: AudioInfo | null;
    cursorTime: number;
    isPlaying: boolean;
    loopEnabled: boolean;
    /** Duration of the active selection, seconds; absent when there is none. */
    selectionSeconds: number | null;
    /** Span of the visible viewport, seconds. */
    viewSpanSeconds: number;
    onSkipStart: () => void;
    onPlayToggle: () => void;
    onSkipEnd: () => void;
    onLoopToggle?: () => void;
    /** Starts a microphone recording; absent when the browser cannot capture. */
    onStartRecording?: () => void;
    /** Whether a take is currently being captured. */
    recording: boolean;
    /** Seconds captured so far; shown inside the record pill while recording. */
    recordingElapsedSeconds: number;
    inspectorOpen: boolean;
    onToggleInspector: () => void;
    exportOpen: boolean;
    onToggleExportFigure: () => void;
    /** Opens the audio-export dialog; absent hides the affordance. */
    onExportAudio?: () => void;
    audioExportOpen: boolean;
  }

  let {
    audio,
    cursorTime,
    isPlaying,
    loopEnabled,
    selectionSeconds,
    viewSpanSeconds,
    onSkipStart,
    onPlayToggle,
    onSkipEnd,
    onLoopToggle,
    onStartRecording,
    recording,
    recordingElapsedSeconds,
    inspectorOpen,
    onToggleInspector,
    exportOpen,
    onToggleExportFigure,
    onExportAudio,
    audioExportOpen
  }: Props = $props();

  const samplePosition = $derived(
    audio ? Math.round(cursorTime * audio.sampleRate).toLocaleString() : null
  );
</script>

<footer class="transport" role="group" aria-label="Transport" data-testid="transport">
  <div class="controls">
    <button
      type="button"
      class="icon-button"
      aria-label="Skip to start"
      title="Skip to start"
      disabled={!audio}
      onclick={onSkipStart}
    >
      <IconSkipBack aria-hidden="true" />
    </button>
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
    <button
      type="button"
      class="icon-button"
      aria-label="Skip to end"
      title="Skip to end"
      disabled={!audio}
      onclick={onSkipEnd}
    >
      <IconSkipForward aria-hidden="true" />
    </button>
    <button
      type="button"
      class="icon-button loop"
      class:on={loopEnabled}
      aria-label="Loop playback"
      aria-pressed={loopEnabled}
      title="Loop playback"
      disabled={!audio || !onLoopToggle}
      onclick={onLoopToggle}
    >
      <IconRepeat aria-hidden="true" />
    </button>
  </div>

  {#if onStartRecording}
    <button
      type="button"
      class="record-pill"
      class:recording
      data-testid="record"
      aria-label={recording ? `Recording, ${formatTime(recordingElapsedSeconds)}` : 'Start recording'}
      disabled={recording}
      onclick={() => onStartRecording?.()}
    >
      {#if recording}
        <span class="rec-dot" aria-hidden="true"></span>
        <span>{formatTime(recordingElapsedSeconds)}</span>
      {:else}
        <IconMic aria-hidden="true" />
        <span>Record</span>
      {/if}
    </button>
  {/if}

  <div class="divider" aria-hidden="true"></div>

  <div class="timecode" data-testid="cursor-readout" data-cursor-time={cursorTime.toFixed(6)}>
    {formatTime(cursorTime)}<small>/ {audio ? formatTime(audio.duration) : '—'}</small>
  </div>

  <div class="divider" aria-hidden="true"></div>

  <div class="status">
    {#if selectionSeconds !== null}
      <span class="field"><span class="k">selection</span><span class="v">{formatTime(selectionSeconds)} s</span></span>
    {/if}
    <span class="field"><span class="k">view</span><span class="v">{formatTime(viewSpanSeconds)} s</span></span>
    {#if audio && samplePosition !== null}
      <span class="field"><span class="k">sample</span><span class="v">{samplePosition}</span></span>
    {/if}
    <span class="field">
      <span class="v">{audio ? `${audio.sampleRate.toFixed(0)} Hz / ${audio.channels} ch` : 'No audio'}</span>
    </span>
  </div>

  <div class="spacer"></div>

  <div class="actions">
    {#if onExportAudio}
      <button
        type="button"
        class="chip-button"
        data-testid="open-audio-export"
        disabled={!audio}
        aria-pressed={audioExportOpen}
        onclick={onExportAudio}
      >
        <IconAudioLines aria-hidden="true" />
        <span>Export audio</span>
      </button>
    {/if}
    <button
      type="button"
      class="chip-button"
      data-testid="open-export"
      disabled={!audio}
      aria-pressed={exportOpen}
      onclick={onToggleExportFigure}
    >
      <IconImage aria-hidden="true" />
      <span>Export figure</span>
    </button>
    <button
      type="button"
      class="chip-button"
      class:on={inspectorOpen}
      data-testid="inspector-toggle"
      aria-pressed={inspectorOpen}
      onclick={onToggleInspector}
    >
      <IconPanelRight aria-hidden="true" />
      <span>Inspector</span>
    </button>
  </div>
</footer>

<style>
  .transport {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    min-height: 3rem;
    padding: 0.4rem 0.75rem;
    border-top: 1px solid var(--chrome-strong);
    background: var(--panel);
    color: var(--text);
  }

  .controls {
    display: flex;
    align-items: center;
    gap: 0.2rem;
  }

  .icon-button {
    width: 2.1rem;
    height: 2.1rem;
    display: grid;
    place-items: center;
    border: 1px solid transparent;
    border-radius: var(--radius-md);
    background: transparent;
    color: var(--muted);
    font-size: 1rem;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  .icon-button:hover:not(:disabled) {
    background: var(--panel-soft);
    color: var(--text);
  }

  .icon-button:disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .icon-button.play {
    width: 2.4rem;
    height: 2.4rem;
    border-radius: 999px;
    color: var(--accent-strong);
    border-color: color-mix(in oklab, var(--accent) 45%, var(--chrome-strong));
    background: var(--accent-tint);
    font-size: 1.1rem;
  }

  .icon-button.play:hover:not(:disabled) {
    background: color-mix(in oklab, var(--accent) 24%, var(--panel-soft));
  }

  .icon-button.loop.on {
    color: var(--accent-strong);
    background: var(--accent-tint);
    border-color: color-mix(in oklab, var(--accent) 40%, var(--chrome-strong));
  }

  .record-pill {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    border: 1px solid var(--chrome-strong);
    border-radius: 999px;
    background: transparent;
    color: var(--text);
    min-height: 1.9rem;
    padding: 0.2rem 0.7rem;
    font-size: 0.78rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    font-variant-numeric: tabular-nums;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  .record-pill:hover:not(:disabled) {
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
    background: var(--panel-soft);
  }

  .record-pill :global(svg) {
    color: var(--danger);
    font-size: 0.95rem;
    flex: none;
  }

  .record-pill.recording {
    border-color: var(--danger);
    color: var(--danger);
    background: color-mix(in oklab, var(--danger) 14%, transparent);
  }

  .record-pill:disabled {
    cursor: not-allowed;
  }

  .record-pill:disabled:not(.recording) {
    opacity: 0.5;
  }

  .rec-dot {
    width: 0.5rem;
    height: 0.5rem;
    flex: none;
    border-radius: 50%;
    background: var(--danger);
    animation: rec-blink 1.1s ease-in-out infinite;
  }

  @keyframes rec-blink {
    0%,
    100% {
      opacity: 1;
    }
    50% {
      opacity: 0.35;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .rec-dot {
      animation: none;
    }
  }

  .divider {
    width: 1px;
    align-self: stretch;
    margin: 0.25rem 0;
    background: var(--chrome-strong);
  }

  .timecode {
    min-width: 8.5rem;
    color: var(--text);
    font-variant-numeric: tabular-nums;
    font-size: 0.95rem;
    white-space: nowrap;
  }

  .timecode small {
    margin-left: 0.3rem;
    color: var(--muted);
    font-size: 0.72rem;
  }

  .status {
    display: flex;
    align-items: center;
    gap: 0.85rem;
    min-width: 0;
    color: var(--muted);
    font-size: 0.76rem;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .field {
    display: inline-flex;
    align-items: baseline;
    gap: 0.3rem;
  }

  .field .k {
    color: var(--muted);
    text-transform: uppercase;
    font-size: 0.66rem;
    letter-spacing: 0.03em;
  }

  .field .v {
    color: var(--text);
  }

  .spacer {
    flex: 1 1 auto;
    min-width: 0.5rem;
  }

  .actions {
    display: flex;
    align-items: center;
    gap: 0.4rem;
    flex: none;
  }

  .chip-button {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    min-height: 1.8rem;
    padding: 0.2rem 0.6rem;
    font-size: 0.78rem;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  .chip-button :global(svg) {
    font-size: 0.95rem;
  }

  .chip-button:hover:not(:disabled) {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  .chip-button:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .chip-button.on {
    color: var(--accent-strong);
    border-color: color-mix(in oklab, var(--accent) 45%, var(--chrome-strong));
    background: var(--accent-tint);
  }

  @media (max-width: 900px) {
    .status .field:not(:last-child) {
      display: none;
    }
  }
</style>
