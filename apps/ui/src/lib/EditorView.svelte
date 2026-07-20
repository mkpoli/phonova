<script lang="ts">
  import IconArrowLeft from '~icons/lucide/arrow-left';
  import IconFolderOpen from '~icons/lucide/folder-open';
  import IconSun from '~icons/lucide/sun';
  import IconMoon from '~icons/lucide/moon';
  import AudioExportDialog from './AudioExportDialog.svelte';
  import ExportDialog from './ExportDialog.svelte';
  import RecordingSwitcher from './RecordingSwitcher.svelte';
  import RecordingsRail from './RecordingsRail.svelte';
  import InspectorPanel from './InspectorPanel.svelte';
  import LevelMeter from './LevelMeter.svelte';
  import OverviewStrip from './OverviewStrip.svelte';
  import PalettePicker from './PalettePicker.svelte';
  import ReadoutBar from './ReadoutBar.svelte';
  import SpectrogramPane from './SpectrogramPane.svelte';
  import TierPane from './TierPane.svelte';
  import TimeRuler from './TimeRuler.svelte';
  import Transport from './Transport.svelte';
  import GradientEditor from './GradientEditor.svelte';
  import VoiceReportCard from './VoiceReportCard.svelte';
  import WaveformPane from './WaveformPane.svelte';
  import { registerCommands } from './commands.svelte';
  import { chordFromEvent, getKeyBindings } from './keybindings.svelte';
  import { BUILTIN_PALETTES, newRampTemplate, type CustomRamp, type PaletteSelection } from './palette';
  import {
    clampViewport,
    defaultOverlayParams,
    defaultViewport,
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
    sampleRate: number;
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
    /** Whether the active ramp renders reversed (floor in the ceiling color). */
    paletteInvert: boolean;
    customRamps: CustomRamp[];
    onFile: (file: File) => void;
    onPlayToggle: () => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
    onPaletteChange: (palette: PaletteSelection) => void;
    /** Flips the palette-inversion flag. */
    onPaletteInvertToggle: () => void;
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
    /** Whether the transport repeats the active playback range; absent hides
     *  the loop control (the host has no loop-capable playback engine). */
    loopEnabled?: boolean;
    onLoopToggle?: () => void;
  }

  let {
    client,
    audio,
    annotationId,
    cursorTime,
    isPlaying,
    theme,
    palette,
    paletteInvert,
    customRamps,
    onFile,
    onPlayToggle,
    onThemeChange,
    onPaletteChange,
    onPaletteInvertToggle,
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
    recordingElapsedSeconds = 0,
    loopEnabled = false,
    onLoopToggle
  }: Props = $props();

  let fileInput = $state<HTMLInputElement | null>(null);

  function takeFileList(files: FileList | null) {
    const file = files?.item(0);
    if (file) onFile(file);
  }

  function skipToStart() {
    onCursorChange?.(0);
  }

  function skipToEnd() {
    if (audio) onCursorChange?.(audio.duration);
  }

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
  let railOpen = $state(true);

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

  // The recordings rail steps out of the way once measurement starts, per the
  // owner's standing question about screen space while deep in a session: each
  // fresh selection (not a held one) collapses it, but a manual reopen during
  // that same selection sticks — this only fires again on the next fresh one.
  let hadSelection = false;
  $effect(() => {
    const hasSelection = selection !== null;
    if (hasSelection && !hadSelection) railOpen = false;
    hadSelection = hasSelection;
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

  // Praat's "Play window" (Shift-Tab in Praat mode): always plays the whole
  // visible window, ignoring any selection, and always restarts rather than
  // toggling to a stop on a second press — unlike playPause/handleTransportToggle.
  function playWindow() {
    if (!audio) return;
    onCursorChange?.(viewport.t0);
    onPlaySelection?.(viewport.t0, viewport.t1);
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

  const keyBindings = getKeyBindings();

  // The `editor` scope's rebindable actions, keyed by command id. Which chord
  // fires which of these is entirely data (`keyBindings`, mode-dependent) —
  // this map only says what each command *does*, never what key it is.
  const editorActions: Record<string, () => void> = {
    playPause: handleTransportToggle,
    playWindow,
    fitFile,
    zoomToSelection: fitSelectionOrFile,
    toggleInspector: () => {
      inspectorOpen = !inspectorOpen;
    },
    toggleWaveform,
    clearSelection,
    exportFigure: () => {
      if (audio) exportOpen = !exportOpen;
    }
  };

  function handleKeydown(event: KeyboardEvent) {
    if (event.target instanceof HTMLInputElement || event.target instanceof HTMLSelectElement) return;
    // Escape closes whichever overlay sits on top before it does anything
    // else; this is baseline modal behavior, not a rebindable command.
    if (event.key === 'Escape' && (selection || voiceReportOpen)) {
      event.preventDefault();
      if (voiceReportOpen) voiceReportOpen = false;
      else clearSelection();
      return;
    }
    if (!keyBindings) return;
    const commandId = keyBindings.commandForChord('editor', chordFromEvent(event));
    const action = commandId ? editorActions[commandId] : undefined;
    if (!action) return;
    event.preventDefault();
    action();
  }

  const hasSelection = () => selection !== null;
  const hasAudio = () => audio !== null;

  registerCommands([
    {
      id: 'playPause',
      title: 'Play / pause',
      group: 'Playback',
      shortcut: () => keyBindings?.labelFor('playPause') ?? 'Space',
      keywords: ['transport', 'stop', 'play selection', 'play visible'],
      enabled: hasAudio,
      run: handleTransportToggle
    },
    {
      id: 'playWindow',
      title: 'Play visible window',
      group: 'Playback',
      shortcut: () => keyBindings?.labelFor('playWindow') ?? '',
      keywords: ['transport', 'play view', 'praat', 'play window'],
      enabled: hasAudio,
      run: playWindow
    },
    {
      id: 'fitFile',
      title: 'Fit whole file',
      group: 'View',
      shortcut: () => keyBindings?.labelFor('fitFile') ?? '0',
      keywords: ['zoom out', 'reset zoom', 'overview'],
      enabled: hasAudio,
      run: fitFile
    },
    {
      id: 'zoomToSelection',
      title: 'Zoom to selection',
      group: 'View',
      shortcut: () => keyBindings?.labelFor('zoomToSelection') ?? 'F',
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
      shortcut: () => keyBindings?.labelFor('toggleInspector') ?? 'I',
      keywords: ['parameters', 'panel'],
      run: () => {
        inspectorOpen = !inspectorOpen;
      }
    },
    {
      id: 'toggleRecordingsRail',
      title: 'Toggle recordings rail',
      group: 'View',
      keywords: ['recordings', 'compare', 'corpus', 'panel'],
      enabled: () => (recordings?.length ?? 0) > 0,
      run: () => {
        railOpen = !railOpen;
      }
    },
    {
      id: 'toggleWaveform',
      title: 'Toggle waveform pane',
      group: 'View',
      shortcut: () => keyBindings?.labelFor('toggleWaveform') ?? 'W',
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
      shortcut: () => keyBindings?.labelFor('clearSelection') ?? 'Esc',
      enabled: hasSelection,
      run: clearSelection
    },
    {
      id: 'exportFigure',
      title: 'Export figure',
      group: 'Figures',
      api: ['buildFigure', 'exportFigure'],
      shortcut: () => keyBindings?.labelFor('exportFigure') ?? 'E',
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
    ...BUILTIN_PALETTES.map((p) => ({
      id: `colormap${p.name}`,
      title: `Spectrogram palette: ${p.label}`,
      group: 'Appearance' as const,
      keywords: ['colormap', 'color', p.label.toLowerCase()],
      run: () => onPaletteChange({ kind: 'builtin', name: p.name })
    })),
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

      <div class="crumb-spacer"></div>

      <label class="crumb-import" title="Open a recording">
        <IconFolderOpen aria-hidden="true" />
        <span>Open</span>
        <input
          bind:this={fileInput}
          data-testid="file-input"
          type="file"
          accept=".wav,audio/wav,audio/x-wav,.aiff,.aif,audio/aiff,.flac,audio/flac"
          onchange={(event) => takeFileList(event.currentTarget.files)}
        />
      </label>

      <PalettePicker
        {palette}
        {customRamps}
        onSelect={onPaletteChange}
        onNewRamp={openNewRamp}
        onEditRamp={openEditRamp}
      />

      <button
        type="button"
        class="icon-button"
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
    </nav>

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
    {#if recordings && recordings.length > 0 && onSwitchRecording}
      <RecordingsRail
        {client}
        {theme}
        recordings={recordings.map((rec) => ({
          mediaId: rec.mediaId,
          name: rec.name,
          duration: rec.duration,
          sampleRate: rec.sampleRate,
          audioId: rec.audioId
        }))}
        currentRecordingId={currentRecordingId ?? null}
        open={railOpen}
        onToggle={() => (railOpen = !railOpen)}
        onSwitch={(mediaId) => onSwitchRecording?.(mediaId)}
      />
    {/if}

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

    {#if audio}
      <LevelMeter {client} audioId={audio.id} duration={audio.duration} {cursorTime} {isPlaying} />
    {/if}

    {#if inspectorOpen}
      <InspectorPanel
        params={overlayParams}
        stats={overlayStats}
        {readout}
        {formantMeans}
        onClose={() => (inspectorOpen = false)}
      />
    {/if}
  </div>

  <Transport
    {audio}
    {cursorTime}
    {isPlaying}
    {loopEnabled}
    selectionSeconds={selection ? selection.t1 - selection.t0 : null}
    viewSpanSeconds={viewport.t1 - viewport.t0}
    onSkipStart={skipToStart}
    onPlayToggle={handleTransportToggle}
    onSkipEnd={skipToEnd}
    {onLoopToggle}
    {onStartRecording}
    {recording}
    {recordingElapsedSeconds}
    {inspectorOpen}
    onToggleInspector={() => (inspectorOpen = !inspectorOpen)}
    {exportOpen}
    onToggleExportFigure={() => (exportOpen = !exportOpen)}
    onExportAudio={onExportAudio && (() => (audioExportOpen = !audioExportOpen))}
    {audioExportOpen}
  />

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
    top: 3rem;
    right: 0.85rem;
    max-height: calc(100% - 4rem);
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

  .crumb-spacer {
    flex: 1 1 auto;
  }

  .crumb-import {
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
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .crumb-import:hover {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  .crumb-import :global(svg) {
    font-size: 0.95rem;
  }

  .crumb-import input {
    display: none;
  }

  .icon-button {
    display: grid;
    place-items: center;
    width: 1.9rem;
    height: 1.9rem;
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-sm);
    background: var(--panel-soft);
    color: var(--text);
    font-size: 1rem;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .icon-button:hover {
    background: var(--panel);
    border-color: color-mix(in oklab, var(--accent) 32%, var(--chrome-strong));
  }

  /* Four possible columns — recordings rail, timeline, level meter, inspector —
     each conditionally rendered; an absent one leaves its `auto` track at zero
     width, so the timeline's `minmax(0, 1fr)` always absorbs whatever the
     chrome around it is not using. */
  .workspace {
    flex: 1 1 auto;
    min-height: 0;
    display: grid;
    grid-template-columns: auto minmax(0, 1fr) auto auto;
  }

  .timeline {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: 1.5rem minmax(9rem, 22vh) minmax(12rem, 1fr) minmax(7rem, 32vh);
    overflow: hidden;
    touch-action: none;
  }

</style>
