<script lang="ts">
  import type {
    AudioInfo,
    CoreClientLike,
    OverlayParams,
    OverlayStats,
    Selection,
    SpectrogramTileRequest,
    ViewportState,
    WasmColormapName
  } from './types';
  import {
    applyCanvasSize,
    cssVar,
    FrameTimeMonitor,
    hexToRgb01,
    makeProgram,
    measureCanvasTarget,
    slippyTransform,
    type DrawnViewport
  } from './rendering';
  import FrequencyRuler from './FrequencyRuler.svelte';
  import SelectionLayer from './SelectionLayer.svelte';
  import TrackOverlay from './TrackOverlay.svelte';

  interface Props {
    client: CoreClientLike | null;
    audio: AudioInfo | null;
    viewport: ViewportState;
    cursorTime: number;
    theme: 'light' | 'dark';
    colormap: WasmColormapName;
    overlayParams: OverlayParams;
    onOverlayStats?: (stats: OverlayStats) => void;
    selection?: Selection | null;
    onSelectionChange?: (selection: Selection | null) => void;
    onSeek?: (time: number) => void;
  }

  let {
    client,
    audio,
    viewport,
    cursorTime,
    theme,
    colormap,
    overlayParams,
    onOverlayStats,
    selection = null,
    onSelectionChange,
    onSeek
  }: Props = $props();
  let canvas = $state<HTMLCanvasElement | null>(null);
  let notice = $state('');
  let usingCanvas2d = $state(false);
  let renderToken = $state(0);
  // Advances on every transform draw (instant CSS remap or fresh raster). Stamped
  // straight onto the canvas rather than through reactive state: the viewport
  // effect calls it synchronously, and a tracked read-modify-write here would
  // retrigger the effect. The e2e reads these to assert the pane tracks the
  // viewport within the frame budget and stays in step with the other panes.
  let drawGen = 0;

  const cache = new Map<string, ImageBitmap>();
  const monitor = new FrameTimeMonitor();
  const tileSeconds = 2;

  // The viewport the current canvas pixels were rasterized for. Null until the
  // first tile lands, when the canvas carries no transform.
  let base: DrawnViewport | null = null;
  // Generation of the most recent fetch; a tile resolved from an older
  // generation is dropped so a superseded pan never overwrites fresh imagery.
  let reqGen = 0;
  let fetchScheduled = false;

  function liveViewport(): DrawnViewport {
    return { t0: viewport.t0, t1: viewport.t1, vLo: viewport.f0, vHi: viewport.f1 };
  }

  // Redraw the existing pixels immediately by remapping them with a CSS
  // transform: waveform, spectrogram, and overlays all follow the one shared
  // viewport, so they move as a single rigid sheet with no worker round-trip.
  function applyTransform() {
    if (!canvas) return;
    canvas.style.transform = base ? slippyTransform(base, liveViewport(), 'freq') : 'none';
    drawGen += 1;
    canvas.setAttribute('data-draw-generation', String(drawGen));
    canvas.setAttribute('data-draw-time', performance.now().toFixed(2));
  }

  function scheduleFetch() {
    if (fetchScheduled) return;
    fetchScheduled = true;
    requestAnimationFrame(() => {
      fetchScheduled = false;
      void fetchFreshTile();
    });
  }

  // A new recording invalidates the transform anchor: the old bitmap belongs to
  // a different signal and must not be stretched over the new viewport.
  $effect(() => {
    audio?.id;
    base = null;
    if (canvas) canvas.style.transform = 'none';
  });

  $effect(() => {
    if (!canvas) return;
    const observer = new ResizeObserver(() => scheduleFetch());
    observer.observe(canvas);
    scheduleFetch();
    return () => observer.disconnect();
  });

  $effect(() => {
    viewport.t0;
    viewport.t1;
    viewport.f0;
    viewport.f1;
    theme;
    colormap;
    applyTransform();
    scheduleFetch();
  });

  async function getTile(width: number, height: number) {
    if (!client || !audio) return null;
    const cssWidth = Math.max(32, Math.floor(width / Math.max(1, window.devicePixelRatio || 1)));
    const cssHeight = Math.max(32, Math.floor(height / Math.max(1, window.devicePixelRatio || 1)));
    const tile0 = Math.floor(viewport.t0 / tileSeconds);
    const tile1 = Math.floor(viewport.t1 / tileSeconds);
    const paramsHash = [
      viewport.f0.toFixed(1),
      viewport.f1.toFixed(1),
      cssWidth,
      cssHeight,
      colormap,
      theme
    ].join(':');
    const key = `${String(audio.id)}:spec:${tile0}:${tile1}:${paramsHash}`;
    const cached = cache.get(key);
    if (cached) return cached;
    const req: SpectrogramTileRequest = {
      t0: viewport.t0,
      t1: viewport.t1,
      f0: viewport.f0,
      f1: viewport.f1,
      widthPx: cssWidth,
      heightPx: cssHeight,
      windowLength: 0.005,
      maxFrequency: 5000,
      timeStep: 0.002,
      frequencyStep: 20,
      dynamicRangeDb: 70,
      colormap,
      theme: theme === 'dark' ? 'Dark' : 'Light'
    };
    const bitmap = await client.spectrogramTile(audio.id, req);
    cache.set(key, bitmap);
    return bitmap;
  }

  // The bitmap currently on the canvas and the pixel size it was drawn at, so a
  // pan that reuses the same tile skips the re-raster and lets the CSS transform
  // carry the motion — the crisp draw runs only when the tile itself changes.
  let displayed: ImageBitmap | null = null;
  let displayedW = 0;
  let displayedH = 0;

  async function fetchFreshTile() {
    if (!canvas) return;
    const gen = ++reqGen;
    const requested = liveViewport();
    // Measure the target size without touching the backing store yet: a
    // resize's new CSS box already stretches the still-displayed bitmap (like
    // an image), so nothing goes blank while this awaits a fresh tile.
    const { width, height, dpr } = measureCanvasTarget(canvas);
    const bitmap = await getTile(width, height);
    // Dropped: a newer pan or zoom already superseded this request.
    if (gen !== reqGen || !canvas) return;
    if (!bitmap) {
      applyCanvasSize(canvas, width, height);
      drawEmpty(width, height);
      displayed = null;
      displayedW = width;
      displayedH = height;
      return;
    }
    // Same tile and canvas size: the current pixels are already correct for
    // `base`, and the transform maps them onto the live viewport. Nothing to do.
    if (bitmap === displayed && width === displayedW && height === displayedH) return;
    // Resize the backing store only now, in the same tick as the redraw, so
    // the canvas is never cleared without fresh pixels ready to fill it.
    applyCanvasSize(canvas, width, height);
    drawBitmap(width, height, dpr, bitmap);
    displayed = bitmap;
    displayedW = width;
    displayedH = height;
    // The fresh pixels represent the viewport at request time; re-apply the
    // transform so any motion since then still shows without a flash.
    base = requested;
    applyTransform();
    renderToken += 1;
  }

  function drawBitmap(width: number, height: number, dpr: number, bitmap: ImageBitmap) {
    if (usingCanvas2d) {
      drawCanvas2d(width, height, dpr, bitmap);
      return;
    }
    const start = performance.now();
    try {
      drawWebgl(width, height, bitmap);
    } catch {
      usingCanvas2d = true;
      notice = 'Canvas fallback active';
      drawCanvas2d(width, height, dpr, bitmap);
    }
    const elapsed = performance.now() - start;
    if (!usingCanvas2d && monitor.record(elapsed)) {
      usingCanvas2d = true;
      notice = 'Canvas fallback active';
    }
  }

  function drawEmpty(width: number, height: number) {
    const ctx = canvas?.getContext('2d');
    if (!ctx) return;
    ctx.fillStyle = cssVar('--canvas', '#f8fafc');
    ctx.fillRect(0, 0, width, height);
  }

  function drawCanvas2d(width: number, height: number, dpr: number, bitmap: ImageBitmap) {
    const ctx = canvas?.getContext('2d');
    if (!ctx) return;
    ctx.fillStyle = cssVar('--canvas', '#f8fafc');
    ctx.fillRect(0, 0, width, height);
    ctx.imageSmoothingEnabled = false;
    ctx.drawImage(bitmap, 0, 0, width, height);
    drawCursor2d(ctx, width, height, dpr);
  }

  function drawCursor2d(ctx: CanvasRenderingContext2D, width: number, height: number, dpr: number) {
    if (!audio || cursorTime < viewport.t0 || cursorTime > viewport.t1) return;
    const x = ((cursorTime - viewport.t0) / (viewport.t1 - viewport.t0)) * width;
    ctx.strokeStyle = cssVar('--warn', '#b45309');
    ctx.lineWidth = Math.max(1, dpr);
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, height);
    ctx.stroke();
  }

  function drawWebgl(width: number, height: number, bitmap: ImageBitmap) {
    const gl = canvas?.getContext('webgl2', { antialias: false, preserveDrawingBuffer: true });
    if (!gl) throw new Error('WebGL2 unavailable');
    const vertex = `#version 300 es
      in vec2 a_pos;
      out vec2 v_uv;
      void main() {
        v_uv = vec2((a_pos.x + 1.0) * 0.5, 1.0 - (a_pos.y + 1.0) * 0.5);
        gl_Position = vec4(a_pos, 0.0, 1.0);
      }`;
    const fragment = `#version 300 es
      precision mediump float;
      uniform sampler2D u_tile;
      in vec2 v_uv;
      out vec4 out_color;
      void main() {
        out_color = texture(u_tile, v_uv);
      }`;
    const program = makeProgram(gl, vertex, fragment);
    const background = hexToRgb01(cssVar('--canvas', '#f8fafc'));
    gl.viewport(0, 0, width, height);
    gl.clearColor(background[0], background[1], background[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(program);
    const texture = gl.createTexture();
    gl.activeTexture(gl.TEXTURE0);
    gl.bindTexture(gl.TEXTURE_2D, texture);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.NEAREST);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
    gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
    gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA, gl.RGBA, gl.UNSIGNED_BYTE, bitmap);
    const vertices = new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STATIC_DRAW);
    const posLoc = gl.getAttribLocation(program, 'a_pos');
    gl.enableVertexAttribArray(posLoc);
    gl.vertexAttribPointer(posLoc, 2, gl.FLOAT, false, 0, 0);
    gl.uniform1i(gl.getUniformLocation(program, 'u_tile'), 0);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    gl.deleteBuffer(buffer);
    gl.deleteTexture(texture);
    gl.deleteProgram(program);
  }
</script>

<section class="pane">
  {#key usingCanvas2d}
    <canvas
      bind:this={canvas}
      class="canvas"
      data-testid="spectrogram-canvas"
      data-render-token={renderToken}
      data-draw-generation="0"
      data-draw-time="0"
    ></canvas>
  {/key}
  <TrackOverlay {client} {audio} {viewport} {theme} params={overlayParams} onStats={onOverlayStats} />
  <FrequencyRuler {viewport} />
  {#if audio && onSelectionChange}
    <SelectionLayer
      {viewport}
      mode="box"
      {selection}
      onChange={onSelectionChange}
      {onSeek}
    />
  {/if}
  <div class="pane-label">Spectrogram</div>
  {#if notice}
    <div class="notice">{notice}</div>
  {/if}
</section>

<style>
  .pane {
    position: relative;
    min-height: 16rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--canvas);
    overflow: hidden;
  }

  .canvas {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 16rem;
    transform-origin: 0 0;
    will-change: transform;
  }

  .pane-label,
  .notice {
    position: absolute;
    z-index: 2;
    top: 0.4rem;
    font-size: 0.75rem;
    pointer-events: none;
    padding: 0.1rem 0.4rem;
    border-radius: 4px;
    background: var(--chip-bg);
    color: var(--chip-fg);
    box-shadow: 0 0 0 1px var(--chip-ring);
  }

  .pane-label {
    left: 0.6rem;
  }

  .notice {
    right: 0.6rem;
    color: var(--warn);
  }
</style>
