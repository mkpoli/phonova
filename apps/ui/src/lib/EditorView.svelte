<script lang="ts">
  import ExportDialog from './ExportDialog.svelte';
  import InspectorPanel from './InspectorPanel.svelte';
  import OverviewStrip from './OverviewStrip.svelte';
  import ReadoutBar from './ReadoutBar.svelte';
  import SpectrogramPane from './SpectrogramPane.svelte';
  import TierPane from './TierPane.svelte';
  import TransportBar from './TransportBar.svelte';
  import VoiceReportCard from './VoiceReportCard.svelte';
  import WaveformPane from './WaveformPane.svelte';
  import { registerCommands } from './commands.svelte';
  import {
    clampViewport,
    defaultOverlayParams,
    defaultViewport,
    formatTime,
    type AudioInfo,
    type CoreClientLike,
    type OverlayParams,
    type OverlayStats,
    type Selection,
    type SelectionReadout,
    type ViewportState,
    type VoiceReportData,
    type WasmColormapName
  } from './types';

  interface RecordingChoice {
    mediaId: number;
    name: string;
  }

  interface Props {
    client: CoreClientLike | null;
    audio: AudioInfo | null;
    annotationId: bigint | null;
    cursorTime: number;
    isPlaying: boolean;
    theme: 'light' | 'dark';
    colormap: WasmColormapName;
    onFile: (file: File) => void;
    onPlayToggle: () => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
    onColormapChange: (colormap: WasmColormapName) => void;
    onCursorChange?: (time: number) => void;
    onAnnotationChange?: (id: bigint) => void;
    onExit?: () => void;
    projectName?: string;
    recordings?: RecordingChoice[];
    currentRecordingId?: number | null;
    onSwitchRecording?: (mediaId: number) => void;
    onPlaySelection?: (t0: number, t1: number) => void;
    /** Starts a microphone recording; absent when the browser cannot capture. */
    onStartRecording?: () => void;
    /** Whether a take is currently being captured. */
    recording?: boolean;
  }

  let {
    client,
    audio,
    annotationId,
    cursorTime,
    isPlaying,
    theme,
    colormap,
    onFile,
    onPlayToggle,
    onThemeChange,
    onColormapChange,
    onCursorChange,
    onAnnotationChange,
    onExit,
    projectName,
    recordings,
    currentRecordingId,
    onSwitchRecording,
    onPlaySelection,
    onStartRecording,
    recording = false
  }: Props = $props();

  let viewport = $state<ViewportState>(defaultViewport());
  let overlayParams = $state<OverlayParams>(defaultOverlayParams());
  let overlayStats = $state<OverlayStats>({ pitchMaxHz: 0, formantMaxHz: 0 });
  let inspectorOpen = $state(true);
  let exportOpen = $state(false);

  let selection = $state<Selection | null>(null);
  let readout = $state<SelectionReadout | null>(null);
  let formantMeans = $state<number[] | null>(null);
  let voiceReportOpen = $state(false);
  let voiceReport = $state<VoiceReportData | null>(null);
  let voiceReportLoading = $state(false);

  $effect(() => {
    const duration = audio?.duration ?? 1;
    viewport = defaultViewport(duration);
    // A new recording invalidates any selection anchored in the old signal.
    selection = null;
  });

  // Selection readout: every value is an engine query over the box, so the bar
  // shows exactly what a script reading the same API returns.
  $effect(() => {
    const sel = selection;
    const id = audio?.id;
    const pitch = overlayParams.pitch;
    const intensityFloor = overlayParams.intensity.floorHz;
    if (!client || id === undefined || !sel) {
      readout = null;
      return;
    }
    let cancelled = false;
    client
      .selectionReadout(
        id,
        sel.t0,
        sel.t1,
        sel.f0,
        sel.f1,
        pitch.floorHz,
        pitch.ceilingHz,
        intensityFloor
      )
      .then((result) => {
        if (!cancelled) readout = result;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  // Provisional tracked-formant means, fetched only while the tracking toggle is
  // on (the raw Burg display is the default until T2.6 closes).
  $effect(() => {
    const sel = selection;
    const id = audio?.id;
    const formant = overlayParams.formant;
    if (!client || id === undefined || !sel || !formant.smoothed) {
      formantMeans = null;
      return;
    }
    let cancelled = false;
    client
      .formantSpanMeans(id, formant.ceilingHz, formant.maxFormants, true, sel.t0, sel.t1)
      .then((means) => {
        if (!cancelled) formantMeans = Array.from(means);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  function handleSelectionChange(next: Selection | null) {
    selection = next;
    if (!next) {
      readout = null;
      formantMeans = null;
    }
  }

  function clearSelection() {
    selection = null;
    readout = null;
    formantMeans = null;
    voiceReportOpen = false;
  }

  function zoomToSelection() {
    if (!selection) return;
    setViewport({
      ...viewport,
      t0: selection.t0,
      t1: selection.t1,
      f0: selection.mode === 'box' ? selection.f0 : viewport.f0,
      f1: selection.mode === 'box' ? selection.f1 : viewport.f1
    });
  }

  function playSelection() {
    if (!selection) return;
    onCursorChange?.(selection.t0);
    onPlaySelection?.(selection.t0, selection.t1);
  }

  async function openVoiceReport() {
    if (!client || !audio || !selection) return;
    voiceReportOpen = true;
    voiceReportLoading = true;
    voiceReport = null;
    const sel = selection;
    try {
      voiceReport = await client.voiceReport(
        audio.id,
        sel.t0,
        sel.t1,
        overlayParams.pitch.floorHz,
        overlayParams.pitch.ceilingHz
      );
    } catch {
      voiceReport = null;
    } finally {
      voiceReportLoading = false;
    }
  }

  function setViewport(next: ViewportState) {
    viewport = clampViewport(next, audio?.duration ?? 1);
  }

  function fitFile() {
    if (!audio) return;
    setViewport(defaultViewport(audio.duration));
  }

  // `F` frames the current selection when one is set, and otherwise falls back
  // to the whole file, so a single key serves both the DAW "fit" gestures.
  function fitSelectionOrFile() {
    if (selection) zoomToSelection();
    else fitFile();
  }

  function zoomHorizontal(factor: number, anchorRatio: number) {
    if (!audio) return;
    const span = viewport.t1 - viewport.t0;
    const anchor = viewport.t0 + span * anchorRatio;
    const nextSpan = span * factor;
    setViewport({
      ...viewport,
      t0: anchor - nextSpan * anchorRatio,
      t1: anchor + nextSpan * (1 - anchorRatio)
    });
  }

  function scrollHorizontal(deltaSeconds: number) {
    setViewport({ ...viewport, t0: viewport.t0 + deltaSeconds, t1: viewport.t1 + deltaSeconds });
  }

  function zoomVertical(factor: number) {
    const f1 = Math.max(200, Math.min(12000, viewport.f1 * factor));
    setViewport({ ...viewport, ampScale: viewport.ampScale / factor, f1 });
  }

  function handleWheel(event: WheelEvent) {
    if (!audio) return;
    // Always swallow the wheel: a Ctrl/Cmd wheel is how a macOS trackpad pinch
    // arrives, and the browser would otherwise page-zoom the whole app.
    event.preventDefault();
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const anchorRatio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    if (event.altKey) {
      zoomVertical(event.deltaY < 0 ? 0.86 : 1.16);
      return;
    }
    if (event.shiftKey) {
      const span = viewport.t1 - viewport.t0;
      scrollHorizontal((event.deltaY / 600) * span);
      return;
    }
    // Plain wheel and Ctrl/Cmd wheel (the trackpad pinch) both drive time zoom
    // anchored on the pointer. Every pane reads the one shared viewport, so the
    // waveform and spectrogram stay locked to the same time axis.
    zoomHorizontal(event.deltaY < 0 ? 0.82 : 1.22, anchorRatio);
  }

  function handlePointer(event: PointerEvent) {
    if (!audio || event.buttons !== 1) return;
    const rect = (event.currentTarget as HTMLElement).getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    const time = viewport.t0 + ratio * (viewport.t1 - viewport.t0);
    onCursorChange?.(time);
  }

  function handleKeydown(event: KeyboardEvent) {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement) return;
    if (event.key === 'Escape' && (selection || voiceReportOpen)) {
      event.preventDefault();
      if (voiceReportOpen) voiceReportOpen = false;
      else clearSelection();
      return;
    }
    if (event.code === 'Space') {
      event.preventDefault();
      onPlayToggle();
    } else if (event.key === '0') {
      event.preventDefault();
      fitFile();
    } else if (event.key.toLowerCase() === 'f') {
      event.preventDefault();
      fitSelectionOrFile();
    } else if (event.key.toLowerCase() === 'i') {
      event.preventDefault();
      inspectorOpen = !inspectorOpen;
    } else if (event.key.toLowerCase() === 'e' && audio) {
      event.preventDefault();
      exportOpen = !exportOpen;
    }
  }

  const hasSelection = () => selection !== null;
  const hasAudio = () => audio !== null;

  registerCommands([
    {
      id: 'playPause',
      title: 'Play / pause',
      group: 'Playback',
      shortcut: 'Space',
      keywords: ['transport', 'stop'],
      enabled: hasAudio,
      run: () => onPlayToggle()
    },
    {
      id: 'fitFile',
      title: 'Fit whole file',
      group: 'View',
      shortcut: '0',
      keywords: ['zoom out', 'reset zoom', 'overview'],
      enabled: hasAudio,
      run: fitFile
    },
    {
      id: 'zoomToSelection',
      title: 'Zoom to selection',
      group: 'View',
      shortcut: 'F',
      keywords: ['fit selection'],
      enabled: hasSelection,
      run: zoomToSelection
    },
    {
      id: 'zoomIn',
      title: 'Zoom in',
      group: 'View',
      shortcut: 'Wheel / pinch',
      keywords: ['time zoom', 'ctrl wheel'],
      enabled: hasAudio,
      run: () => zoomHorizontal(0.8, 0.5)
    },
    {
      id: 'zoomOut',
      title: 'Zoom out',
      group: 'View',
      shortcut: 'Wheel / pinch',
      keywords: ['time zoom', 'ctrl wheel'],
      enabled: hasAudio,
      run: () => zoomHorizontal(1.25, 0.5)
    },
    {
      id: 'zoomFrequency',
      title: 'Zoom frequency / amplitude',
      group: 'View',
      shortcut: 'Alt+wheel',
      keywords: ['vertical zoom', 'frequency range', 'amplitude'],
      enabled: hasAudio,
      run: () => zoomVertical(0.86)
    },
    {
      id: 'toggleInspector',
      title: 'Toggle inspector',
      group: 'View',
      shortcut: 'I',
      keywords: ['parameters', 'panel'],
      run: () => {
        inspectorOpen = !inspectorOpen;
      }
    },
    {
      id: 'togglePitchTrack',
      title: 'Toggle pitch track',
      group: 'Analysis',
      api: ['pitchTrack'],
      keywords: ['f0', 'overlay'],
      run: () => {
        overlayParams.pitch.show = !overlayParams.pitch.show;
      }
    },
    {
      id: 'toggleFormantTrack',
      title: 'Toggle formant track',
      group: 'Analysis',
      api: ['formantTrack'],
      keywords: ['overlay'],
      run: () => {
        overlayParams.formant.show = !overlayParams.formant.show;
      }
    },
    {
      id: 'toggleIntensityTrack',
      title: 'Toggle intensity track',
      group: 'Analysis',
      api: ['intensityTrack'],
      keywords: ['overlay', 'db'],
      run: () => {
        overlayParams.intensity.show = !overlayParams.intensity.show;
      }
    },
    {
      id: 'toggleFormantTracking',
      title: 'Toggle formant tracking',
      group: 'Analysis',
      api: ['formantSpanMeans'],
      keywords: ['smoothed', 'burg'],
      run: () => {
        overlayParams.formant.smoothed = !overlayParams.formant.smoothed;
      }
    },
    {
      id: 'voiceReport',
      title: 'Voice report over selection',
      group: 'Analysis',
      api: ['voiceReport'],
      keywords: ['jitter', 'shimmer', 'hnr'],
      enabled: hasSelection,
      run: () => void openVoiceReport()
    },
    {
      id: 'playSelection',
      title: 'Play selection',
      group: 'Selection',
      enabled: hasSelection,
      run: playSelection
    },
    {
      id: 'clearSelection',
      title: 'Clear selection',
      group: 'Selection',
      shortcut: 'Esc',
      enabled: hasSelection,
      run: clearSelection
    },
    {
      id: 'exportFigure',
      title: 'Export figure',
      group: 'Figures',
      api: ['buildFigure', 'exportFigure'],
      shortcut: 'E',
      keywords: ['svg', 'pdf', 'png', 'save image'],
      enabled: hasAudio,
      run: () => {
        exportOpen = !exportOpen;
      }
    },
    {
      id: 'colormapViridis',
      title: 'Spectrogram palette: Viridis',
      group: 'Appearance',
      keywords: ['colormap', 'color'],
      run: () => onColormapChange('Viridis')
    },
    {
      id: 'colormapMagma',
      title: 'Spectrogram palette: Magma',
      group: 'Appearance',
      keywords: ['colormap', 'color'],
      run: () => onColormapChange('Magma')
    },
    {
      id: 'colormapGrayscale',
      title: 'Spectrogram palette: Grayscale',
      group: 'Appearance',
      keywords: ['colormap', 'grayscale', 'print', 'publication'],
      run: () => onColormapChange('Grayscale')
    },
    {
      id: 'closeRecording',
      title: 'Close recording',
      group: 'Project',
      keywords: ['back', 'corpus', 'exit'],
      enabled: () => onExit !== undefined,
      run: () => onExit?.()
    }
  ]);
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="editor"
  data-testid="editor"
  data-visible-start={viewport.t0.toFixed(6)}
  data-visible-end={viewport.t1.toFixed(6)}
  data-visible-freq={viewport.f1.toFixed(6)}
  data-cursor-time={cursorTime.toFixed(6)}
>
    <nav class="breadcrumb" data-testid="editor-breadcrumb">
      {#if onExit}
        <button type="button" class="crumb-back" data-testid="back-corpus" onclick={() => onExit?.()}>
          ← {projectName ?? 'Project'}
        </button>
      {/if}
      {#if recordings && recordings.length > 1 && onSwitchRecording}
        <select
          class="recording-switch"
          aria-label="Switch recording"
          data-testid="recording-switch"
          value={currentRecordingId ?? undefined}
          onchange={(event) => onSwitchRecording?.(Number(event.currentTarget.value))}
        >
          {#each recordings as recording (recording.mediaId)}
            <option value={recording.mediaId}>{recording.name}</option>
          {/each}
        </select>
      {:else}
        <span class="crumb-current">{audio?.name ?? ''}</span>
      {/if}
      {#if onStartRecording}
        <button
          type="button"
          class="crumb-record"
          data-testid="record"
          disabled={recording}
          onclick={() => onStartRecording?.()}
        >
          Record
        </button>
      {/if}
    </nav>

  <TransportBar
    {audio}
    {cursorTime}
    {isPlaying}
    {theme}
    {colormap}
    {onFile}
    {onPlayToggle}
    {onThemeChange}
    {onColormapChange}
  />

  <OverviewStrip {client} {audio} {viewport} {theme} onViewportChange={setViewport} />

  {#if selection}
    <ReadoutBar
      {selection}
      {readout}
      {formantMeans}
      showFormants={overlayParams.formant.smoothed}
      onPlay={playSelection}
      onZoom={zoomToSelection}
      onVoiceReport={openVoiceReport}
      onClear={clearSelection}
    />
  {/if}

  <div class="workspace">
    <main class="timeline" data-testid="timeline" onwheel={handleWheel} onpointerdown={handlePointer} onpointermove={handlePointer}>
      <WaveformPane
        {client}
        {audio}
        {viewport}
        {cursorTime}
        {theme}
        {selection}
        onSelectionChange={handleSelectionChange}
        onSeek={(time) => onCursorChange?.(time)}
      />
      <SpectrogramPane
        {client}
        {audio}
        {viewport}
        {cursorTime}
        {theme}
        {colormap}
        {overlayParams}
        onOverlayStats={(stats) => (overlayStats = stats)}
        {selection}
        onSelectionChange={handleSelectionChange}
        onSeek={(time) => onCursorChange?.(time)}
      />
      <TierPane
        {client}
        audioId={audio?.id ?? null}
        {annotationId}
        audioDuration={audio?.duration ?? 0}
        sampleRate={audio?.sampleRate ?? 0}
        {viewport}
        {cursorTime}
        onSeek={(time) => onCursorChange?.(time)}
        {onAnnotationChange}
      />
    </main>

    {#if inspectorOpen}
      <InspectorPanel params={overlayParams} stats={overlayStats} onClose={() => (inspectorOpen = false)} />
    {/if}
  </div>

  <footer class="status">
    <span>t {formatTime(cursorTime)}</span>
    <span class="status-right">
      <span>{audio ? `${audio.sampleRate.toFixed(0)} Hz / ${audio.channels} ch` : 'No audio'}</span>
      <button
        type="button"
        class="inspector-toggle"
        data-testid="open-export"
        disabled={!audio}
        aria-pressed={exportOpen}
        onclick={() => (exportOpen = !exportOpen)}
      >
        Export figure
      </button>
      <button
        type="button"
        class="inspector-toggle"
        data-testid="inspector-toggle"
        aria-pressed={inspectorOpen}
        onclick={() => (inspectorOpen = !inspectorOpen)}
      >
        Inspector {inspectorOpen ? '▸' : '◂'}
      </button>
    </span>
  </footer>

  {#if exportOpen && audio}
    <ExportDialog
      {client}
      {audio}
      {annotationId}
      {viewport}
      {overlayParams}
      appTheme={theme}
      {colormap}
      onClose={() => (exportOpen = false)}
    />
  {/if}

  {#if voiceReportOpen}
    <VoiceReportCard report={voiceReport} loading={voiceReportLoading} onClose={() => (voiceReportOpen = false)} />
  {/if}
</div>

<style>
  .editor {
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto auto auto minmax(0, 1fr) auto;
    background: var(--chrome);
    color: var(--text);
  }

  .breadcrumb {
    display: flex;
    align-items: center;
    gap: 0.6rem;
    min-height: 2.1rem;
    padding: 0.25rem 0.75rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--chrome);
    font-size: 0.82rem;
  }

  .crumb-back {
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.15rem 0.5rem;
  }

  .crumb-back:hover {
    background: var(--panel);
  }

  .crumb-current {
    color: var(--muted);
  }

  .crumb-record {
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.15rem 0.5rem;
  }

  .crumb-record::before {
    content: '';
    width: 0.5rem;
    height: 0.5rem;
    border-radius: 50%;
    background: #dc2626;
  }

  .crumb-record:disabled {
    opacity: 0.5;
  }

  .crumb-record:hover:not(:disabled) {
    background: var(--panel);
  }

  .recording-switch {
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.15rem 0.4rem;
    max-width: 18rem;
  }

  .workspace {
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .timeline {
    min-height: 0;
    display: grid;
    grid-template-rows: minmax(9rem, 22vh) minmax(12rem, 1fr) minmax(7rem, 32vh);
    overflow: hidden;
    touch-action: none;
  }

  .status {
    min-height: 2rem;
    padding: 0.35rem 0.75rem;
    display: flex;
    justify-content: space-between;
    gap: 1rem;
    border-top: 1px solid var(--chrome-strong);
    color: var(--muted);
    background: var(--panel);
    font-size: 0.82rem;
    font-variant-numeric: tabular-nums;
  }

  .status-right {
    display: flex;
    align-items: center;
    gap: 0.75rem;
  }

  .inspector-toggle {
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.15rem 0.5rem;
    font-size: 0.78rem;
  }
</style>
