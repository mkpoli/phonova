<script lang="ts">
  import IconArrowLeft from '~icons/lucide/arrow-left';
  import IconImage from '~icons/lucide/image';
  import IconAudioLines from '~icons/lucide/audio-lines';
  import IconPanelRight from '~icons/lucide/panel-right';
  import AudioExportDialog from './AudioExportDialog.svelte';
  import ExportDialog from './ExportDialog.svelte';
  import RecordingSwitcher from './RecordingSwitcher.svelte';
  import InspectorPanel from './InspectorPanel.svelte';
  import OverviewStrip from './OverviewStrip.svelte';
  import ReadoutBar from './ReadoutBar.svelte';
  import SpectrogramPane from './SpectrogramPane.svelte';
  import TierPane from './TierPane.svelte';
  import TimeRuler from './TimeRuler.svelte';
  import TransportBar from './TransportBar.svelte';
  import GradientEditor from './GradientEditor.svelte';
  import VoiceReportCard from './VoiceReportCard.svelte';
  import WaveformPane from './WaveformPane.svelte';
  import { registerCommands } from './commands.svelte';
  import { newRampTemplate, type CustomRamp, type PaletteSelection } from './palette';
  import {
    clampViewport,
    defaultOverlayParams,
    defaultViewport,
    formatTime,
    type AudioInfo,
    type CoreClientLike,
    type OverlayParams,
    type LibraryNode,
    type OverlayStats,
    type AudioExportOptions,
    type AudioExportRequest,
    type Selection,
    type SelectionReadout,
    type ViewportState,
    type VoiceReportData,
    type AudioId
  } from './types';

  interface RecordingChoice {
    mediaId: number;
    name: string;
    duration: number;
    audioId: AudioId | null;
    hasAnnotation: boolean;
  }

  interface Props {
    client: CoreClientLike | null;
    audio: AudioInfo | null;
    annotationId: bigint | null;
    cursorTime: number;
    isPlaying: boolean;
    theme: 'light' | 'dark';
    palette: PaletteSelection;
    customRamps: CustomRamp[];
    onFile: (file: File) => void;
    onPlayToggle: () => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
    onPaletteChange: (palette: PaletteSelection) => void;
    /** Persists a created or edited custom ramp. */
    onSaveRamp: (ramp: CustomRamp) => void;
    /** Removes a custom ramp by id. */
    onDeleteRamp: (id: string) => void;
    onCursorChange?: (time: number) => void;
    onAnnotationChange?: (id: bigint | null) => void;
    onExit?: () => void;
    projectName?: string;
    recordings?: RecordingChoice[];
    /** The library tree, so the recording switcher mirrors the corpus's grouping. */
    groups?: LibraryNode[];
    currentRecordingId?: number | null;
    onSwitchRecording?: (mediaId: number) => void;
    onRenameRecording?: (mediaId: number, name: string) => void;
    onPlaySelection?: (t0: number, t1: number) => void;
    /** Plays a box selection through the engine's band filter; resolves when the
     *  rendered preview finishes sounding. */
    onPlayFilteredSelection?: (t0: number, t1: number, f0: number, f1: number) => Promise<void> | void;
    /** Encodes and downloads the whole recording or the selection as WAV;
     *  absent hides the audio-export affordances. */
    onExportAudio?: (request: AudioExportRequest) => void;
    /** Starts a microphone recording; absent when the browser cannot capture. */
    onStartRecording?: () => void;
    /** Whether a take is currently being captured. */
    recording?: boolean;
    /** Seconds captured so far; shown inside the REC pill while `recording`. */
    recordingElapsedSeconds?: number;
  }

  let {
    client,
    audio,
    annotationId,
    cursorTime,
    isPlaying,
    theme,
    palette,
    customRamps,
    onFile,
    onPlayToggle,
    onThemeChange,
    onPaletteChange,
    onSaveRamp,
    onDeleteRamp,
    onCursorChange,
    onAnnotationChange,
    onExit,
    projectName,
    recordings,
    groups,
    currentRecordingId,
    onSwitchRecording,
    onRenameRecording,
    onPlaySelection,
    onPlayFilteredSelection,
    onExportAudio,
    onStartRecording,
    recording = false,
    recordingElapsedSeconds = 0
  }: Props = $props();

  let audioExportOpen = $state(false);

  // Gradient editor. While a draft is open the spectrogram previews it live, so
  // the pane shows the draft ramp (recolor path) rather than the committed
  // palette. Cancelling drops the draft; saving commits and selects it.
  let editingRamp = $state<CustomRamp | null>(null);
  let editingExisting = $state(false);
  const activePalette = $derived<PaletteSelection>(
    editingRamp ? { kind: 'custom', ramp: editingRamp } : palette
  );

  function openNewRamp() {
    editingExisting = false;
    editingRamp = newRampTemplate();
  }

  function openEditRamp(ramp: CustomRamp) {
    editingExisting = true;
    editingRamp = { ...ramp, stops: ramp.stops.map((s) => ({ ...s })) };
  }

  function saveRamp(ramp: CustomRamp) {
    onSaveRamp(ramp);
    onPaletteChange({ kind: 'custom', ramp });
    editingRamp = null;
  }

  function deleteRamp(id: string) {
    onDeleteRamp(id);
    editingRamp = null;
  }

  // Resolve the dialog's choice into concrete signal coordinates: the whole file
  // from zero, or the live selection's span (band-limited to its box only when
  // the user asked and the selection carries frequency bounds).
  function resolveAudioExport(options: AudioExportOptions): AudioExportRequest | null {
    if (!audio) return null;
    if (options.scope === 'selection' && selection) {
      const box = selection.mode === 'box';
      return {
        scope: 'selection',
        t0: selection.t0,
        t1: selection.t1,
        f0: box ? selection.f0 : 0,
        f1: box ? selection.f1 : 0,
        bits: options.bits,
        filtered: options.filtered && box
      };
    }
    return {
      scope: 'whole',
      t0: 0,
      t1: audio.duration,
      f0: 0,
      f1: 0,
      bits: options.bits,
      filtered: false
    };
  }

  let switcher = $state<{ show: () => void } | null>(null);

  // The ceiling and amplitude a reset chip returns to; the chips appear the
  // instant the live value departs these.
  const DEFAULT_CEILING = defaultViewport().f1;
  const DEFAULT_AMP = defaultViewport().ampScale;

  let waveformVisible = $state(true);
  let filteredPlaying = $state(false);

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

  async function playSelection() {
    const sel = selection;
    if (!sel) return;
    // A box plays band-filtered through the engine; a time selection plays the
    // plain slice from the transport. The band case renders a preview and does
    // not move the file cursor.
    if (sel.mode === 'box' && onPlayFilteredSelection) {
      filteredPlaying = true;
      try {
        await onPlayFilteredSelection(sel.t0, sel.t1, sel.f0, sel.f1);
      } finally {
        filteredPlaying = false;
      }
      return;
    }
    onCursorChange?.(sel.t0);
    onPlaySelection?.(sel.t0, sel.t1);
  }

  // Transport Play / Space plays what the user is looking at, in priority order:
  // an active selection's time span, else the visible viewport when zoomed in,
  // else the whole file from the cursor. A box selection plays its time span
  // unfiltered here — the band-filtered preview stays on the readout's own
  // affordance.
  function handleTransportToggle() {
    if (isPlaying) {
      onPlayToggle();
      return;
    }
    if (selection) {
      onCursorChange?.(selection.t0);
      onPlaySelection?.(selection.t0, selection.t1);
      return;
    }
    const zoomedIn = !!audio && viewport.t1 - viewport.t0 < audio.duration - 1e-6;
    if (zoomedIn) {
      onCursorChange?.(viewport.t0);
      onPlaySelection?.(viewport.t0, viewport.t1);
      return;
    }
    onPlayToggle();
  }

  // Double-click on a pane: inside the live selection zooms to it, empty pane
  // space fits the whole file.
  function handleDoubleZoom(intent: 'zoom' | 'fit') {
    if (intent === 'zoom' && selection) zoomToSelection();
    else fitFile();
  }

  function scaleFrequencyCeiling(factor: number) {
    const f1 = Math.max(200, Math.min(20000, viewport.f1 * factor));
    setViewport({ ...viewport, f1 });
  }

  function resetFrequencyCeiling() {
    setViewport({ ...viewport, f1: DEFAULT_CEILING });
  }

  function scaleAmplitude(factor: number) {
    setViewport({ ...viewport, ampScale: Math.max(0.25, Math.min(8, viewport.ampScale * factor)) });
  }

  function resetAmplitude() {
    setViewport({ ...viewport, ampScale: DEFAULT_AMP });
  }

  function resetVerticalScale() {
    setViewport({ ...viewport, f1: DEFAULT_CEILING, ampScale: DEFAULT_AMP });
  }

  function toggleWaveform() {
    waveformVisible = !waveformVisible;
  }

  function handleTierInterval(t0: number, t1: number) {
    selection = { t0, t1, f0: viewport.f0, f1: viewport.f1, mode: 'time' };
    onCursorChange?.(t0);
    onPlaySelection?.(t0, t1);
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
    // The single-key view shortcuts below never combine with a modifier; a
    // Ctrl/Cmd chord belongs to the app (UI scale, palette), so let it pass.
    if (event.ctrlKey || event.metaKey || event.altKey) return;
    if (event.code === 'Space') {
      event.preventDefault();
      handleTransportToggle();
    } else if (event.key === '0') {
      event.preventDefault();
      fitFile();
    } else if (event.key.toLowerCase() === 'f') {
      event.preventDefault();
      fitSelectionOrFile();
    } else if (event.key.toLowerCase() === 'i') {
      event.preventDefault();
      inspectorOpen = !inspectorOpen;
    } else if (event.key.toLowerCase() === 'w') {
      event.preventDefault();
      toggleWaveform();
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
      keywords: ['transport', 'stop', 'play selection', 'play visible'],
      enabled: hasAudio,
      run: handleTransportToggle
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
      id: 'toggleWaveform',
      title: 'Toggle waveform pane',
      group: 'View',
      shortcut: 'W',
      keywords: ['waveform', 'ghost', 'overlay', 'envelope', 'hide'],
      enabled: hasAudio,
      run: toggleWaveform
    },
    {
      id: 'resetVerticalScale',
      title: 'Reset vertical scale',
      group: 'View',
      keywords: ['amplitude', 'frequency ceiling', 'gain', 'reset zoom'],
      enabled: hasAudio,
      run: resetVerticalScale
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
      id: 'exportAudio',
      title: 'Export audio (WAV)',
      group: 'Figures',
      api: ['exportSpanWav', 'exportBandFilteredSpanWav'],
      keywords: ['wav', 'save audio', 'selection', 'clip', 'download audio', 'sound'],
      enabled: () => hasAudio() && onExportAudio !== undefined,
      run: () => {
        audioExportOpen = true;
      }
    },
    {
      id: 'colormapPhonia',
      title: 'Spectrogram palette: Phonia',
      group: 'Appearance',
      keywords: ['colormap', 'color', 'default', 'brand'],
      run: () => onPaletteChange({ kind: 'builtin', name: 'Phonia' })
    },
    {
      id: 'colormapViridis',
      title: 'Spectrogram palette: Viridis',
      group: 'Appearance',
      keywords: ['colormap', 'color', 'color-blind', 'cvd', 'accessible'],
      run: () => onPaletteChange({ kind: 'builtin', name: 'Viridis' })
    },
    {
      id: 'colormapMagma',
      title: 'Spectrogram palette: Magma',
      group: 'Appearance',
      keywords: ['colormap', 'color'],
      run: () => onPaletteChange({ kind: 'builtin', name: 'Magma' })
    },
    {
      id: 'colormapInferno',
      title: 'Spectrogram palette: Inferno',
      group: 'Appearance',
      keywords: ['colormap', 'color'],
      run: () => onPaletteChange({ kind: 'builtin', name: 'Inferno' })
    },
    {
      id: 'colormapPlasma',
      title: 'Spectrogram palette: Plasma',
      group: 'Appearance',
      keywords: ['colormap', 'color'],
      run: () => onPaletteChange({ kind: 'builtin', name: 'Plasma' })
    },
    {
      id: 'colormapCividis',
      title: 'Spectrogram palette: Cividis',
      group: 'Appearance',
      keywords: ['colormap', 'color', 'color-blind', 'cvd', 'accessible'],
      run: () => onPaletteChange({ kind: 'builtin', name: 'Cividis' })
    },
    {
      id: 'colormapGrayscale',
      title: 'Spectrogram palette: Grayscale',
      group: 'Appearance',
      keywords: ['colormap', 'grayscale', 'print', 'publication'],
      run: () => onPaletteChange({ kind: 'builtin', name: 'Grayscale' })
    },
    {
      id: 'colormapNewRamp',
      title: 'New custom spectrogram ramp…',
      group: 'Appearance',
      keywords: ['colormap', 'gradient', 'custom', 'palette', 'editor'],
      run: openNewRamp
    },
    {
      id: 'switchRecording',
      title: 'Switch recording',
      group: 'Project',
      keywords: ['open', 'take', 'corpus', 'change recording'],
      enabled: () => !!onSwitchRecording && (recordings?.length ?? 0) > 1,
      run: () => switcher?.show()
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
          <IconArrowLeft aria-hidden="true" />
          <span>{projectName ?? 'Project'}</span>
        </button>
      {/if}
      {#if recordings && recordings.length > 0 && onSwitchRecording}
        <RecordingSwitcher
          bind:this={switcher}
          {client}
          {theme}
          {recordings}
          {groups}
          currentRecordingId={currentRecordingId ?? null}
          onSwitch={(mediaId) => onSwitchRecording?.(mediaId)}
          onRename={onRenameRecording}
        />
      {:else}
        <span class="crumb-current">{recordings?.[0]?.name ?? audio?.name ?? ''}</span>
      {/if}
      {#if onStartRecording}
        <button
          type="button"
          class="crumb-record"
          class:recording
          data-testid="record"
          aria-label={recording ? `Recording, ${formatTime(recordingElapsedSeconds)}` : 'Start recording'}
          disabled={recording}
          onclick={() => onStartRecording?.()}
        >
          <span class="rec-dot" aria-hidden="true"></span>
          <span>{recording ? formatTime(recordingElapsedSeconds) : 'REC'}</span>
        </button>
      {/if}
    </nav>

  <TransportBar
    {audio}
    {cursorTime}
    {isPlaying}
    {theme}
    {palette}
    {customRamps}
    {onFile}
    onPlayToggle={handleTransportToggle}
    {onThemeChange}
    onPaletteChange={onPaletteChange}
    onNewRamp={openNewRamp}
    onEditRamp={openEditRamp}
  />

  {#if editingRamp}
    <div class="ramp-editor-slot" data-testid="ramp-editor-slot">
      {#key editingRamp.id}
        <GradientEditor
          ramp={editingRamp}
          existing={editingExisting}
          onChange={(next) => (editingRamp = next)}
          onSave={saveRamp}
          onCancel={() => (editingRamp = null)}
          onDelete={deleteRamp}
        />
      {/key}
    </div>
  {/if}

  <OverviewStrip {client} {audio} {viewport} {theme} onViewportChange={setViewport} />

  {#if selection}
    <ReadoutBar
      {selection}
      {readout}
      {formantMeans}
      showFormants={overlayParams.formant.smoothed}
      {filteredPlaying}
      onPlay={playSelection}
      onZoom={zoomToSelection}
      onVoiceReport={openVoiceReport}
      onClear={clearSelection}
    />
  {/if}

  <div class="workspace">
    <main
      class="timeline"
      data-testid="timeline"
      data-waveform-visible={waveformVisible}
      style:grid-template-rows={waveformVisible
        ? '1.5rem minmax(9rem, 22vh) minmax(12rem, 1fr) minmax(7rem, 32vh)'
        : '1.5rem minmax(12rem, 1fr) minmax(7rem, 32vh)'}
      onwheel={handleWheel}
      onpointerdown={handlePointer}
      onpointermove={handlePointer}
    >
      <TimeRuler {viewport} />
      {#if waveformVisible}
        <WaveformPane
          {client}
          {audio}
          {viewport}
          {cursorTime}
          {theme}
          {selection}
          onSelectionChange={handleSelectionChange}
          onSeek={(time) => onCursorChange?.(time)}
          onScaleAmp={scaleAmplitude}
          onResetAmp={resetAmplitude}
          onDoubleZoom={handleDoubleZoom}
        />
      {/if}
      <SpectrogramPane
        {client}
        {audio}
        {viewport}
        {cursorTime}
        {theme}
        palette={activePalette}
        {overlayParams}
        onOverlayStats={(stats) => (overlayStats = stats)}
        {selection}
        onSelectionChange={handleSelectionChange}
        onSeek={(time) => onCursorChange?.(time)}
        onScaleFrequency={scaleFrequencyCeiling}
        onResetFrequency={resetFrequencyCeiling}
        onDoubleZoom={handleDoubleZoom}
        ghostWaveform={!waveformVisible}
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
        onIntervalActivate={handleTierInterval}
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
      {#if onExportAudio}
        <button
          type="button"
          class="inspector-toggle"
          data-testid="open-audio-export"
          disabled={!audio}
          aria-pressed={audioExportOpen}
          onclick={() => (audioExportOpen = !audioExportOpen)}
        >
          <IconAudioLines aria-hidden="true" />
          <span>Export audio</span>
        </button>
      {/if}
      <button
        type="button"
        class="inspector-toggle"
        data-testid="open-export"
        disabled={!audio}
        aria-pressed={exportOpen}
        onclick={() => (exportOpen = !exportOpen)}
      >
        <IconImage aria-hidden="true" />
        <span>Export figure</span>
      </button>
      <button
        type="button"
        class="inspector-toggle"
        class:on={inspectorOpen}
        data-testid="inspector-toggle"
        aria-pressed={inspectorOpen}
        onclick={() => (inspectorOpen = !inspectorOpen)}
      >
        <IconPanelRight aria-hidden="true" />
        <span>Inspector</span>
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
      appPalette={palette}
      onClose={() => (exportOpen = false)}
    />
  {/if}

  {#if audioExportOpen && audio && onExportAudio}
    <AudioExportDialog
      hasSelection={selection !== null}
      isBoxSelection={selection?.mode === 'box'}
      onExport={(options) => {
        const request = resolveAudioExport(options);
        if (request) onExportAudio?.(request);
        audioExportOpen = false;
      }}
      onClose={() => (audioExportOpen = false)}
    />
  {/if}

  {#if voiceReportOpen}
    <VoiceReportCard report={voiceReport} loading={voiceReportLoading} onClose={() => (voiceReportOpen = false)} />
  {/if}
</div>

<style>
  /* Fits the viewport exactly, like a DAW: transport chrome keeps its natural
     height, the workspace takes whatever is left and shrinks panes internally
     (TierPane's row list, InspectorPanel) rather than growing the page. Flex
     stacking — not a fixed-row grid — so an optional row (ReadoutBar) never
     desyncs a row count from the number of children. */
  .editor {
    position: relative;
    height: 100vh;
    height: 100dvh;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    background: var(--chrome);
    color: var(--text);
  }

  .editor > :global(*) {
    flex: none;
  }

  /* The gradient editor floats below the transport, over the workspace, so
     opening it never reflows the panes and the live preview stays visible. */
  .ramp-editor-slot {
    position: absolute;
    top: 5.6rem;
    right: 0.85rem;
    max-height: calc(100% - 6.5rem);
    overflow-y: auto;
    z-index: 40;
  }

  @media (max-width: 720px) {
    .ramp-editor-slot {
      right: 0.5rem;
      left: 0.5rem;
    }
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
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    min-height: 1.6rem;
    padding: 0.2rem 0.55rem;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .crumb-back :global(svg) {
    font-size: 0.95rem;
  }

  .crumb-back:hover {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  .crumb-current {
    color: var(--muted);
  }

  /* Compact REC pill: quiet by default (a dim red hint on the border, a muted
     label), and only reads as loud on hover or while a take is actually
     rolling — the dot itself stays red at rest as the affordance's identity. */
  .crumb-record {
    display: inline-flex;
    align-items: center;
    gap: 0.4rem;
    border: 1px solid color-mix(in oklab, var(--danger) 28%, var(--chrome-strong));
    border-radius: 999px;
    background: transparent;
    color: var(--muted);
    min-height: 1.6rem;
    padding: 0.2rem 0.65rem;
    font-size: 0.76rem;
    font-weight: 600;
    letter-spacing: 0.02em;
    font-variant-numeric: tabular-nums;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  .crumb-record:hover:not(:disabled) {
    border-color: var(--danger);
    color: var(--danger);
    background: color-mix(in oklab, var(--danger) 10%, transparent);
  }

  .crumb-record.recording {
    border-color: var(--danger);
    color: var(--danger);
    background: color-mix(in oklab, var(--danger) 14%, transparent);
  }

  .crumb-record:disabled {
    cursor: not-allowed;
  }

  .crumb-record:disabled:not(.recording) {
    opacity: 0.5;
  }

  .rec-dot {
    width: 0.5rem;
    height: 0.5rem;
    flex: none;
    border-radius: 50%;
    background: var(--danger);
  }

  .crumb-record.recording .rec-dot {
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
    .crumb-record.recording .rec-dot {
      animation: none;
    }
  }

  .workspace {
    flex: 1 1 auto;
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .timeline {
    min-height: 0;
    display: grid;
    grid-template-rows: 1.5rem minmax(9rem, 22vh) minmax(12rem, 1fr) minmax(7rem, 32vh);
    overflow: hidden;
    touch-action: none;
  }

  .status {
    min-height: 2.1rem;
    padding: 0.35rem 0.75rem;
    display: flex;
    align-items: center;
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
    display: inline-flex;
    align-items: center;
    gap: 0.35rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    min-height: 1.6rem;
    padding: 0.2rem 0.55rem;
    font-size: 0.78rem;
    transition:
      background var(--t-fast),
      border-color var(--t-fast),
      color var(--t-fast);
  }

  .inspector-toggle :global(svg) {
    font-size: 0.95rem;
  }

  .inspector-toggle:hover:not(:disabled) {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  .inspector-toggle:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .inspector-toggle.on {
    color: var(--accent-strong);
    border-color: color-mix(in oklab, var(--accent) 45%, var(--chrome-strong));
    background: var(--accent-tint);
  }
</style>
