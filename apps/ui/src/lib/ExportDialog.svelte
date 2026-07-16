<script lang="ts">
  import { untrack } from 'svelte';
  import IconImage from '~icons/lucide/image';
  import IconX from '~icons/lucide/x';
  import IconDownload from '~icons/lucide/download';
  import { zipStore } from './zip';
  import type {
    AudioInfo,
    CoreClientLike,
    FigureColormapName,
    FigureExportFormat,
    FigureLayerToggles,
    FigureLengthUnit,
    FigureSpec,
    FigureThemeName,
    OverlayParams,
    ViewportState,
    WasmColormapName
  } from './types';

  interface Props {
    client: CoreClientLike | null;
    audio: AudioInfo | null;
    annotationId: bigint | null;
    viewport: ViewportState;
    overlayParams: OverlayParams;
    appTheme: 'light' | 'dark';
    colormap: WasmColormapName;
    onClose: () => void;
  }

  let { client, audio, annotationId, viewport, overlayParams, appTheme, colormap, onClose }: Props =
    $props();

  const LAYER_LABELS: Array<{ key: keyof FigureLayerToggles; label: string }> = [
    { key: 'waveform', label: 'Waveform' },
    { key: 'spectrogram', label: 'Spectrogram' },
    { key: 'pitch', label: 'Pitch' },
    { key: 'formant', label: 'Formants' },
    { key: 'intensity', label: 'Intensity' },
    { key: 'tiers', label: 'Tiers' }
  ];

  const FORMATS: Array<{ value: FigureExportFormat; label: string; nativeOnly?: boolean }> = [
    { value: 'svg', label: 'SVG' },
    { value: 'png', label: 'PNG' },
    { value: 'pdf', label: 'PDF', nativeOnly: true },
    { value: 'vega', label: 'Vega-Lite JSON' },
    { value: 'tikz', label: 'TikZ (PGFPlots)' },
    { value: 'typst', label: 'Typst / CeTZ' },
    { value: 'python', label: 'Python (matplotlib)' },
    { value: 'r', label: 'R (ggplot2)' },
    { value: 'julia', label: 'Julia (Makie)' }
  ];

  // The dialog opens seeded from the live editor state, then owns these
  // controls independently; untrack keeps the seed from re-binding to the props.
  let layers = $state<FigureLayerToggles>({
    waveform: true,
    spectrogram: true,
    pitch: true,
    formant: false,
    intensity: false,
    tiers: untrack(() => annotationId) !== null
  });
  let width = $state(16);
  let height = $state(12);
  let unit = $state<FigureLengthUnit>('cm');
  let figTheme = $state<FigureThemeName>(untrack(() => appTheme));
  let palette = $state<FigureColormapName>(
    untrack(() => colormap).toLowerCase() as FigureColormapName
  );
  let format = $state<FigureExportFormat>('svg');

  let svg = $state('');
  let figureJson = $state('');
  let busy = $state(false);
  let error = $state('');
  let previewToken = 0;

  const activeFormat = $derived(FORMATS.find((f) => f.value === format));
  const noLayers = $derived(!LAYER_LABELS.some(({ key }) => layers[key]));

  function buildSpec(): FigureSpec | null {
    if (!audio) return null;
    return {
      audio: Number(audio.id),
      annotation: annotationId !== null ? Number(annotationId) : null,
      t0: viewport.t0,
      t1: viewport.t1,
      f0: viewport.f0,
      f1: viewport.f1,
      layers: { ...layers },
      width,
      height,
      unit,
      theme: figTheme,
      colormap: palette,
      dynamic_range_db: 70,
      max_db: null,
      spectrogram_width_px: 800,
      spectrogram_height_px: 256,
      window_length: 0.005,
      pitch_floor_hz: overlayParams.pitch.floorHz,
      pitch_ceiling_hz: overlayParams.pitch.ceilingHz,
      pitch_unit: 'hertz',
      formant_ceiling_hz: overlayParams.formant.ceilingHz,
      formant_max: overlayParams.formant.maxFormants,
      formant_smoothed: overlayParams.formant.smoothed,
      intensity_floor_hz: overlayParams.intensity.floorHz
    };
  }

  async function refresh() {
    const spec = buildSpec();
    if (!client || !spec || noLayers) {
      svg = '';
      figureJson = '';
      if (noLayers) error = '';
      return;
    }
    const token = ++previewToken;
    busy = true;
    error = '';
    try {
      const json = await client.buildFigure(spec);
      if (token !== previewToken) return;
      figureJson = json;
      const rendered = await client.renderFigureSvg(json);
      if (token !== previewToken) return;
      svg = rendered;
    } catch (caught) {
      if (token !== previewToken) return;
      error = caught instanceof Error ? caught.message : String(caught);
      svg = '';
      figureJson = '';
    } finally {
      if (token === previewToken) busy = false;
    }
  }

  // Re-render the preview whenever any figure option changes. The preview goes
  // through the same SVG backend the export uses, so what shows is what saves.
  $effect(() => {
    // Track every input the spec reads so the effect reruns on any edit.
    void [
      client,
      audio,
      annotationId,
      viewport.t0,
      viewport.t1,
      viewport.f0,
      viewport.f1,
      layers.waveform,
      layers.spectrogram,
      layers.pitch,
      layers.formant,
      layers.intensity,
      layers.tiers,
      width,
      height,
      unit,
      figTheme,
      palette,
      overlayParams
    ];
    void refresh();
  });

  function inchesPerUnit(u: FigureLengthUnit): number {
    if (u === 'cm') return 1 / 2.54;
    if (u === 'pt') return 1 / 72;
    return 1;
  }

  function saveBlob(blob: Blob, name: string) {
    const url = URL.createObjectURL(blob);
    const anchor = document.createElement('a');
    anchor.href = url;
    anchor.download = name;
    anchor.click();
    URL.revokeObjectURL(url);
  }

  function baseName(name: string): string {
    const dot = name.indexOf('.');
    return dot > 0 ? name.slice(0, dot) : name;
  }

  async function downloadPng() {
    if (!svg) return;
    const dpi = 192;
    const w = Math.max(1, Math.round(width * inchesPerUnit(unit) * dpi));
    const h = Math.max(1, Math.round(height * inchesPerUnit(unit) * dpi));
    const blob = new Blob([svg], { type: 'image/svg+xml;charset=utf-8' });
    const url = URL.createObjectURL(blob);
    try {
      const image = new Image();
      image.width = w;
      image.height = h;
      await new Promise<void>((resolve, reject) => {
        image.onload = () => resolve();
        image.onerror = () => reject(new Error('SVG could not be rasterized'));
        image.src = url;
      });
      const canvas = document.createElement('canvas');
      canvas.width = w;
      canvas.height = h;
      const ctx = canvas.getContext('2d');
      if (!ctx) throw new Error('canvas 2D context unavailable');
      ctx.drawImage(image, 0, 0, w, h);
      const png = await new Promise<Blob | null>((resolve) => canvas.toBlob(resolve, 'image/png'));
      if (png) saveBlob(png, 'figure.png');
    } finally {
      URL.revokeObjectURL(url);
    }
  }

  async function download() {
    if (!client || !figureJson) return;
    error = '';
    try {
      if (format === 'png') {
        await downloadPng();
        return;
      }
      const bundle = await client.exportFigure(figureJson, format);
      if (bundle.sidecars.length > 0) {
        const entries = [
          { name: bundle.mainName, bytes: bundle.mainBytes },
          ...bundle.sidecars
        ];
        const zip = zipStore(entries);
        saveBlob(
          new Blob([zip as BlobPart], { type: 'application/zip' }),
          `${baseName(bundle.mainName)}.zip`
        );
      } else {
        saveBlob(new Blob([bundle.mainBytes as BlobPart], { type: bundle.mime }), bundle.mainName);
      }
    } catch (caught) {
      error = caught instanceof Error ? caught.message : String(caught);
    }
  }

  function onKeydown(event: KeyboardEvent) {
    if (event.key === 'Escape') {
      event.stopPropagation();
      onClose();
    }
  }
</script>

<svelte:window onkeydown={onKeydown} />

<section class="export-dialog" data-testid="export-dialog" aria-label="Figure export">
  <header class="head">
    <h2><IconImage aria-hidden="true" />Export figure</h2>
    <button type="button" class="close" data-testid="export-close" aria-label="Close" onclick={onClose}>
      <IconX aria-hidden="true" />
    </button>
  </header>

  <div class="body">
    <div class="preview-column">
      <div class="preview" class:dark={figTheme === 'dark'} data-testid="figure-preview">
        {#if svg}
          <!-- eslint-disable-next-line svelte/no-at-html-tags -->
          {@html svg}
        {:else if noLayers}
          <p class="hint" data-testid="figure-no-layers">Select at least one layer to build the figure.</p>
        {:else}
          <p class="hint">{busy ? 'Rendering…' : 'No preview'}</p>
        {/if}
      </div>
      <!-- Raw preview source: the exact bytes the SVG export writes, kept as
           text so a test can compare it against the download byte for byte. -->
      <div class="raw-source" data-testid="figure-preview-source" hidden>{svg}</div>
    </div>

    <div class="controls">
      <fieldset>
        <legend>Layers</legend>
        {#each LAYER_LABELS as { key, label } (key)}
          <label class="check">
            <input
              type="checkbox"
              data-testid={`figure-layer-${key}`}
              checked={layers[key]}
              disabled={key === 'tiers' && annotationId === null}
              onchange={(event) => (layers = { ...layers, [key]: event.currentTarget.checked })}
            />
            {label}
          </label>
        {/each}
        {#if layers.formant && overlayParams.formant.smoothed}
          <p class="note" data-testid="figure-smoothed-note">
            Formants are Viterbi-smoothed; the export marks them provisional.
          </p>
        {/if}
      </fieldset>

      <fieldset>
        <legend>Size</legend>
        <div class="row">
          <label class="field">
            <span>Width</span>
            <input
              type="number"
              min="1"
              step="0.5"
              data-testid="figure-width"
              value={width}
              onchange={(event) => (width = Number(event.currentTarget.value) || width)}
            />
          </label>
          <label class="field">
            <span>Height</span>
            <input
              type="number"
              min="1"
              step="0.5"
              data-testid="figure-height"
              value={height}
              onchange={(event) => (height = Number(event.currentTarget.value) || height)}
            />
          </label>
          <label class="field">
            <span>Unit</span>
            <select
              data-testid="figure-unit"
              value={unit}
              onchange={(event) => (unit = event.currentTarget.value as FigureLengthUnit)}
            >
              <option value="cm">cm</option>
              <option value="in">in</option>
              <option value="pt">pt</option>
            </select>
          </label>
        </div>
      </fieldset>

      <fieldset>
        <legend>Appearance</legend>
        <div class="row">
          <label class="field">
            <span>Palette</span>
            <select
              data-testid="figure-palette"
              value={palette}
              onchange={(event) => (palette = event.currentTarget.value as FigureColormapName)}
            >
              <option value="viridis">Viridis</option>
              <option value="magma">Magma</option>
              <option value="grayscale">Grayscale (print)</option>
            </select>
          </label>
          <div class="field">
            <span>Preview theme</span>
            <div class="segmented" role="group" aria-label="Preview theme">
              <button
                type="button"
                data-testid="figure-theme-light"
                aria-pressed={figTheme === 'light'}
                class:active={figTheme === 'light'}
                onclick={() => (figTheme = 'light')}>Light</button
              >
              <button
                type="button"
                data-testid="figure-theme-dark"
                aria-pressed={figTheme === 'dark'}
                class:active={figTheme === 'dark'}
                onclick={() => (figTheme = 'dark')}>Dark</button
              >
            </div>
          </div>
        </div>
      </fieldset>

      <fieldset>
        <legend>Format</legend>
        <select
          data-testid="figure-format"
          value={format}
          onchange={(event) => (format = event.currentTarget.value as FigureExportFormat)}
        >
          {#each FORMATS as option (option.value)}
            <option value={option.value}>{option.label}</option>
          {/each}
        </select>
        {#if activeFormat?.nativeOnly}
          <p class="note" data-testid="figure-native-only">
            PDF export runs in the native app. On the web, export SVG and convert, or use PNG.
          </p>
        {/if}
      </fieldset>

      {#if error}
        <p class="error" data-testid="export-error">{error}</p>
      {/if}

      <button
        type="button"
        class="download"
        data-testid="figure-download"
        disabled={!figureJson || activeFormat?.nativeOnly}
        onclick={download}
      >
        <IconDownload aria-hidden="true" /><span>Download</span>
      </button>
    </div>
  </div>
</section>

<style>
  .export-dialog {
    position: fixed;
    top: 3.5rem;
    right: 1rem;
    bottom: 1rem;
    width: min(56rem, calc(100vw - 2rem));
    display: flex;
    flex-direction: column;
    background: var(--panel);
    color: var(--text);
    border: 1px solid var(--chrome-strong);
    border-radius: var(--radius-xl);
    box-shadow: var(--shadow-lg);
    z-index: 20;
    overflow: hidden;
  }

  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 0.6rem 0.9rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--panel-soft);
  }

  .head h2 {
    margin: 0;
    display: flex;
    align-items: center;
    gap: 0.4rem;
    font-size: 0.95rem;
    font-weight: 600;
  }

  .head h2 :global(svg) {
    font-size: 1rem;
    color: var(--accent);
  }

  .close {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border: 1px solid transparent;
    border-radius: var(--radius-sm);
    background: transparent;
    color: var(--muted);
    padding: 0.2rem;
    cursor: pointer;
    transition:
      background var(--t-fast),
      color var(--t-fast);
  }

  .close:hover {
    background: var(--panel);
    color: var(--text);
  }

  .body {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 18rem;
    gap: 0.75rem;
    padding: 0.75rem;
    min-height: 0;
    overflow: auto;
  }

  .preview-column {
    min-width: 0;
  }

  .preview {
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0.75rem;
    border-radius: 8px;
    border: 1px solid var(--chrome-strong);
    background:
      linear-gradient(45deg, rgba(148, 163, 184, 0.12) 25%, transparent 25%) 0 0 / 16px 16px,
      var(--panel-soft);
    min-height: 18rem;
  }

  .preview.dark {
    background: #0b1220;
  }

  .preview :global(svg) {
    width: 100%;
    height: auto;
    max-width: 100%;
  }

  .hint {
    color: var(--muted);
    font-size: 0.85rem;
  }

  .controls {
    display: flex;
    flex-direction: column;
    gap: 0.7rem;
  }

  fieldset {
    border: 1px solid var(--chrome-strong);
    border-radius: 8px;
    padding: 0.55rem 0.7rem 0.7rem;
    margin: 0;
  }

  legend {
    padding: 0 0.35rem;
    font-size: 0.75rem;
    color: var(--muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .check {
    display: flex;
    align-items: center;
    gap: 0.45rem;
    font-size: 0.85rem;
    padding: 0.12rem 0;
  }

  .row {
    display: flex;
    gap: 0.5rem;
    flex-wrap: wrap;
  }

  .field {
    display: flex;
    flex-direction: column;
    gap: 0.2rem;
    font-size: 0.72rem;
    color: var(--muted);
    flex: 1 1 4rem;
  }

  .field input,
  .field select,
  .controls > fieldset select {
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.25rem 0.4rem;
    font-size: 0.82rem;
    width: 100%;
  }

  .segmented {
    display: inline-flex;
    border: 1px solid var(--chrome-strong);
    border-radius: 5px;
    overflow: hidden;
  }

  .segmented button {
    border: none;
    background: var(--panel-soft);
    color: var(--text);
    padding: 0.25rem 0.6rem;
    font-size: 0.8rem;
    cursor: pointer;
  }

  .segmented button.active {
    background: var(--accent);
    color: var(--on-accent);
  }

  .note {
    margin: 0.4rem 0 0;
    font-size: 0.72rem;
    color: var(--muted);
    line-height: 1.35;
  }

  .error {
    margin: 0;
    color: var(--warn);
    font-size: 0.78rem;
  }

  .download {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 0.4rem;
    border: 1px solid var(--action);
    border-radius: var(--radius-md);
    background: var(--action);
    color: #fff;
    padding: 0.5rem 0.6rem;
    font-size: 0.88rem;
    font-weight: 600;
    cursor: pointer;
    transition:
      background var(--t-fast),
      border-color var(--t-fast);
  }

  .download :global(svg) {
    font-size: 1rem;
  }

  .download:hover:not(:disabled) {
    background: var(--action-strong);
    border-color: var(--action-strong);
  }

  .download:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }
</style>
