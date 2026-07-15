<script lang="ts">
  import InspectorPanel from './InspectorPanel.svelte';
  import OverviewStrip from './OverviewStrip.svelte';
  import SpectrogramPane from './SpectrogramPane.svelte';
  import TransportBar from './TransportBar.svelte';
  import WaveformPane from './WaveformPane.svelte';
  import {
    clampViewport,
    defaultOverlayParams,
    defaultViewport,
    formatTime,
    type AudioInfo,
    type CoreClientLike,
    type OverlayParams,
    type OverlayStats,
    type ViewportState,
    type WasmColormapName
  } from './types';

  interface Props {
    client: CoreClientLike | null;
    audio: AudioInfo | null;
    cursorTime: number;
    isPlaying: boolean;
    theme: 'light' | 'dark';
    colormap: WasmColormapName;
    onFile: (file: File) => void;
    onPlayToggle: () => void;
    onThemeChange: (theme: 'light' | 'dark') => void;
    onColormapChange: (colormap: WasmColormapName) => void;
    onCursorChange?: (time: number) => void;
  }

  let {
    client,
    audio,
    cursorTime,
    isPlaying,
    theme,
    colormap,
    onFile,
    onPlayToggle,
    onThemeChange,
    onColormapChange,
    onCursorChange
  }: Props = $props();

  let viewport = $state<ViewportState>(defaultViewport());
  let overlayParams = $state<OverlayParams>(defaultOverlayParams());
  let overlayStats = $state<OverlayStats>({ pitchMaxHz: 0, formantMaxHz: 0 });
  let inspectorOpen = $state(true);

  $effect(() => {
    const duration = audio?.duration ?? 1;
    viewport = defaultViewport(duration);
  });

  function setViewport(next: ViewportState) {
    viewport = clampViewport(next, audio?.duration ?? 1);
  }

  function fitFile() {
    if (!audio) return;
    setViewport(defaultViewport(audio.duration));
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
    event.preventDefault();
    const target = event.currentTarget as HTMLElement;
    const rect = target.getBoundingClientRect();
    const anchorRatio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    if (event.ctrlKey || event.metaKey) {
      zoomVertical(event.deltaY < 0 ? 0.86 : 1.16);
      return;
    }
    if (event.shiftKey) {
      const span = viewport.t1 - viewport.t0;
      scrollHorizontal((event.deltaY / 600) * span);
      return;
    }
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
    if (event.code === 'Space') {
      event.preventDefault();
      onPlayToggle();
    } else if (event.key === '0' || event.key.toLowerCase() === 'f') {
      event.preventDefault();
      fitFile();
    } else if (event.key.toLowerCase() === 'i') {
      event.preventDefault();
      inspectorOpen = !inspectorOpen;
    }
  }
</script>

<svelte:window onkeydown={handleKeydown} />

<div
  class="editor"
  data-testid="editor"
  data-visible-start={viewport.t0.toFixed(6)}
  data-visible-end={viewport.t1.toFixed(6)}
  data-cursor-time={cursorTime.toFixed(6)}
>
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

  <div class="workspace">
    <main class="timeline" data-testid="timeline" onwheel={handleWheel} onpointerdown={handlePointer} onpointermove={handlePointer}>
      <WaveformPane {client} {audio} {viewport} {cursorTime} {theme} />
      <SpectrogramPane
        {client}
        {audio}
        {viewport}
        {cursorTime}
        {theme}
        {colormap}
        {overlayParams}
        onOverlayStats={(stats) => (overlayStats = stats)}
      />
      <div class="tier-slot" aria-hidden="true"></div>
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
        data-testid="inspector-toggle"
        aria-pressed={inspectorOpen}
        onclick={() => (inspectorOpen = !inspectorOpen)}
      >
        Inspector {inspectorOpen ? '▸' : '◂'}
      </button>
    </span>
  </footer>
</div>

<style>
  .editor {
    min-height: 100vh;
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto;
    background: var(--chrome);
    color: var(--text);
  }

  .workspace {
    min-height: 0;
    display: grid;
    grid-template-columns: minmax(0, 1fr) auto;
  }

  .timeline {
    min-height: 0;
    display: grid;
    grid-template-rows: minmax(11rem, 30vh) minmax(16rem, 1fr) 2.5rem;
    overflow: hidden;
    touch-action: none;
  }

  .tier-slot {
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel);
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
