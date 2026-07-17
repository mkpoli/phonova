<script lang="ts">
  import IconRotateCcw from '~icons/lucide/rotate-ccw';
  import type { AudioInfo, CoreClientLike, Selection, ViewportState } from './types';
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
  import SelectionLayer from './SelectionLayer.svelte';

  interface Props {
    client: CoreClientLike | null;
    audio: AudioInfo | null;
    viewport: ViewportState;
    cursorTime: number;
    theme: 'light' | 'dark';
    selection?: Selection | null;
    onSelectionChange?: (selection: Selection | null) => void;
    onSeek?: (time: number) => void;
    /** Multiplies the waveform amplitude scale by `factor`; the caller clamps it. */
    onScaleAmp?: (factor: number) => void;
    /** Restores the amplitude scale to its default. */
    onResetAmp?: () => void;
    /** Double-click intent: zoom to the active selection, or fit the whole file. */
    onDoubleZoom?: (intent: 'zoom' | 'fit') => void;
  }

  let {
    client,
    audio,
    viewport,
    cursorTime,
    theme,
    selection = null,
    onSelectionChange,
    onSeek,
    onScaleAmp,
    onResetAmp,
    onDoubleZoom
  }: Props = $props();

  const ampScaled = $derived(Math.abs(viewport.ampScale - 1) > 1e-3);

  let ampDragging = $state(false);
  let ampLastY = 0;

  function ampPointerDown(event: PointerEvent) {
    if (event.button !== 0 || !onScaleAmp) return;
    event.stopPropagation();
    event.preventDefault();
    ampDragging = true;
    ampLastY = event.clientY;
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
  }

  function ampPointerMove(event: PointerEvent) {
    if (!ampDragging || !onScaleAmp) return;
    const delta = event.clientY - ampLastY;
    ampLastY = event.clientY;
    if (delta === 0) return;
    // Drag up magnifies the waveform, drag down flattens it. Exponential keeps
    // the gesture uniform across the range.
    onScaleAmp(Math.exp(-delta * 0.006));
  }

  function ampPointerUp(event: PointerEvent) {
    if (!ampDragging) return;
    ampDragging = false;
    (event.currentTarget as HTMLElement).releasePointerCapture(event.pointerId);
  }
  let canvas = $state<HTMLCanvasElement | null>(null);
  let notice = $state('');
  let usingCanvas2d = $state(false);
  let renderToken = $state(0);
  // Level of detail the pane is currently drawing: `envelope` is the min/max
  // pyramid at low zoom; `raw` is a sample-accurate polyline once the view holds
  // fewer than one sample per pixel; `raw-dots` adds sample markers past ~6 px
  // per sample, the way a DAW reveals individual samples. Exposed as a data
  // attribute for the zoom-detail e2e.
  let lodMode = $state<'envelope' | 'raw' | 'raw-dots'>('envelope');
  // Advances on every transform draw, stamped straight onto the canvas rather
  // than through reactive state (the viewport effect calls it synchronously). The
  // e2e reads these to assert the waveform tracks the viewport in step with the
  // other panes.
  let drawGen = 0;

  const cache = new Map<string, Float32Array>();
  const monitor = new FrameTimeMonitor();
  const tileSeconds = 2;

  // The viewport the current pixels were rasterized for; the vertical anchor is
  // the amplitude scale. Null until the first slice lands.
  let base: DrawnViewport | null = null;
  let reqGen = 0;
  let fetchScheduled = false;

  function liveViewport(): DrawnViewport {
    return { t0: viewport.t0, t1: viewport.t1, vLo: viewport.ampScale, vHi: 0 };
  }

  function applyTransform() {
    if (!canvas) return;
    canvas.style.transform = base ? slippyTransform(base, liveViewport(), 'amp') : 'none';
    drawGen += 1;
    canvas.setAttribute('data-draw-generation', String(drawGen));
    canvas.setAttribute('data-draw-time', performance.now().toFixed(2));
  }

  function scheduleFetch() {
    if (fetchScheduled) return;
    fetchScheduled = true;
    requestAnimationFrame(() => {
      fetchScheduled = false;
      void fetchFreshSlice();
    });
  }

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
    viewport.ampScale;
    theme;
    applyTransform();
    scheduleFetch();
  });

  async function getWaveform(width: number) {
    if (!client || !audio) return null;
    const px = Math.max(16, Math.floor(width / Math.max(1, window.devicePixelRatio || 1)));
    const tile0 = Math.floor(viewport.t0 / tileSeconds);
    const tile1 = Math.floor(viewport.t1 / tileSeconds);
    const key = `${String(audio.id)}:wave:${tile0}:${tile1}:${px}`;
    const cached = cache.get(key);
    if (cached) return cached;
    const slice = await client.waveformSlice(audio.id, viewport.t0, viewport.t1, px);
    cache.set(key, slice.data);
    return slice.data;
  }

  // The visible span's raw samples, at the source rate. Fetched only at high
  // zoom, where the span is short and the read is cheap; not cached, since a pan
  // shifts the span continuously.
  async function getRawSamples(): Promise<Float32Array | null> {
    if (!client || !audio) return null;
    return client.samplesInRange(audio.id, viewport.t0, viewport.t1);
  }

  // The level of detail the current view calls for, from pixels per source
  // sample across the visible span.
  function lodFor(cssWidth: number): 'envelope' | 'raw' | 'raw-dots' {
    if (!audio || audio.sampleRate <= 0) return 'envelope';
    const samples = (viewport.t1 - viewport.t0) * audio.sampleRate;
    if (samples <= 0) return 'envelope';
    const perSample = cssWidth / samples;
    if (perSample > 6) return 'raw-dots';
    if (perSample >= 1) return 'raw';
    return 'envelope';
  }

  // The slice currently on the canvas and the pixel size it was drawn at, so a
  // pan that reuses the same slice skips the re-raster and lets the CSS
  // transform carry the motion.
  let displayed: Float32Array | null = null;
  let displayedW = 0;
  let displayedH = 0;
  let displayedMode: 'envelope' | 'raw' | 'raw-dots' | null = null;
  // The waveform slice is theme-independent, so a theme switch returns the same
  // cached array and would otherwise skip the redraw — leaving the pane painted
  // in the previous theme's paper colour while every other surface flips. Track
  // the theme the pixels were drawn for and treat a theme change like a resize.
  let displayedTheme: 'light' | 'dark' | null = null;

  async function fetchFreshSlice() {
    if (!canvas) return;
    const gen = ++reqGen;
    const requested = liveViewport();
    // Measure the target size without touching the backing store yet: a
    // resize's new CSS box already stretches the still-displayed bitmap (like
    // an image), so nothing goes blank while this awaits a fresh slice.
    const { width, height, dpr } = measureCanvasTarget(canvas);
    const cssWidth = canvas.clientWidth || Math.floor(width / dpr);
    const mode = lodFor(cssWidth);
    const data = mode === 'envelope' ? await getWaveform(width) : await getRawSamples();
    if (gen !== reqGen || !canvas) return;
    if (!data || data.length === 0) {
      applyCanvasSize(canvas, width, height);
      drawEmpty(width, height);
      displayed = null;
      displayedW = width;
      displayedH = height;
      displayedMode = null;
      return;
    }
    if (
      data === displayed &&
      width === displayedW &&
      height === displayedH &&
      theme === displayedTheme &&
      mode === displayedMode
    )
      return;
    // Resize the backing store only now, in the same tick as the redraw, so
    // the canvas is never cleared without fresh pixels ready to fill it.
    applyCanvasSize(canvas, width, height);
    if (mode === 'envelope') drawSlice(width, height, dpr, data, requested);
    else drawRawSlice(width, height, dpr, data, requested, mode === 'raw-dots');
    displayed = data;
    displayedW = width;
    displayedH = height;
    displayedTheme = theme;
    displayedMode = mode;
    lodMode = mode;
    base = requested;
    applyTransform();
    renderToken += 1;
  }

  function drawSlice(
    width: number,
    height: number,
    dpr: number,
    data: Float32Array,
    vp: DrawnViewport
  ) {
    if (usingCanvas2d) {
      drawCanvas2d(width, height, dpr, data, vp);
      return;
    }
    const start = performance.now();
    try {
      drawWebgl(width, height, data, vp);
    } catch {
      usingCanvas2d = true;
      notice = 'Canvas fallback active';
      drawCanvas2d(width, height, dpr, data, vp);
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

  function drawCanvas2d(
    width: number,
    height: number,
    dpr: number,
    data: Float32Array,
    vp: DrawnViewport
  ) {
    const ctx = canvas?.getContext('2d');
    if (!ctx) return;
    ctx.fillStyle = cssVar('--canvas', '#f8fafc');
    ctx.fillRect(0, 0, width, height);
    ctx.strokeStyle = cssVar('--accent', '#0f766e');
    ctx.lineWidth = Math.max(1, dpr);
    const mid = height / 2;
    const scale = height * 0.44 * vp.vLo;
    const buckets = data.length / 2;
    ctx.beginPath();
    for (let i = 0; i < buckets; i += 1) {
      const x = (i / Math.max(1, buckets - 1)) * width;
      ctx.moveTo(x, mid - data[i * 2 + 1] * scale);
      ctx.lineTo(x, mid - data[i * 2] * scale);
    }
    ctx.stroke();
    drawCursor2d(ctx, width, height, vp);
  }

  function drawCursor2d(
    ctx: CanvasRenderingContext2D,
    width: number,
    height: number,
    vp: DrawnViewport
  ) {
    if (!audio || cursorTime < vp.t0 || cursorTime > vp.t1) return;
    const x = ((cursorTime - vp.t0) / (vp.t1 - vp.t0)) * width;
    ctx.strokeStyle = cssVar('--warn', '#b45309');
    ctx.beginPath();
    ctx.moveTo(x, 0);
    ctx.lineTo(x, height);
    ctx.stroke();
  }

  function drawWebgl(width: number, height: number, data: Float32Array, vp: DrawnViewport) {
    const gl = canvas?.getContext('webgl2', { antialias: false, preserveDrawingBuffer: true });
    if (!gl) throw new Error('WebGL2 unavailable');
    const vertex = `#version 300 es
      in float a_time;
      in float a_amp;
      uniform float u_t0;
      uniform float u_t1;
      uniform float u_amp_scale;
      void main() {
        float x = ((a_time - u_t0) / (u_t1 - u_t0)) * 2.0 - 1.0;
        float y = clamp(a_amp * u_amp_scale, -1.0, 1.0);
        gl_Position = vec4(x, y, 0.0, 1.0);
      }`;
    const fragment = `#version 300 es
      precision mediump float;
      uniform vec3 u_color;
      out vec4 out_color;
      void main() {
        out_color = vec4(u_color, 1.0);
      }`;
    const program = makeProgram(gl, vertex, fragment);
    const vertices = new Float32Array(data.length * 2);
    const buckets = data.length / 2;
    for (let i = 0; i < buckets; i += 1) {
      const time = vp.t0 + ((i + 0.5) / buckets) * (vp.t1 - vp.t0);
      vertices[i * 4] = time;
      vertices[i * 4 + 1] = data[i * 2];
      vertices[i * 4 + 2] = time;
      vertices[i * 4 + 3] = data[i * 2 + 1];
    }
    const background = hexToRgb01(cssVar('--canvas', '#f8fafc'));
    const color = hexToRgb01(cssVar('--accent', '#0f766e'));
    gl.viewport(0, 0, width, height);
    gl.clearColor(background[0], background[1], background[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(program);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STREAM_DRAW);
    const timeLoc = gl.getAttribLocation(program, 'a_time');
    const ampLoc = gl.getAttribLocation(program, 'a_amp');
    gl.enableVertexAttribArray(timeLoc);
    gl.enableVertexAttribArray(ampLoc);
    gl.vertexAttribPointer(timeLoc, 1, gl.FLOAT, false, 8, 0);
    gl.vertexAttribPointer(ampLoc, 1, gl.FLOAT, false, 8, 4);
    gl.uniform1f(gl.getUniformLocation(program, 'u_t0'), vp.t0);
    gl.uniform1f(gl.getUniformLocation(program, 'u_t1'), vp.t1);
    gl.uniform1f(gl.getUniformLocation(program, 'u_amp_scale'), vp.vLo);
    gl.uniform3f(gl.getUniformLocation(program, 'u_color'), color[0], color[1], color[2]);
    gl.drawArrays(gl.LINES, 0, buckets * 2);
    gl.deleteBuffer(buffer);
    gl.deleteProgram(program);
  }

  function drawRawSlice(
    width: number,
    height: number,
    dpr: number,
    samples: Float32Array,
    vp: DrawnViewport,
    dots: boolean
  ) {
    if (usingCanvas2d) {
      drawRawCanvas2d(width, height, dpr, samples, vp, dots);
      return;
    }
    try {
      drawWebglRaw(width, height, samples, vp, dots);
    } catch {
      usingCanvas2d = true;
      notice = 'Canvas fallback active';
      drawRawCanvas2d(width, height, dpr, samples, vp, dots);
    }
  }

  function drawRawCanvas2d(
    width: number,
    height: number,
    dpr: number,
    samples: Float32Array,
    vp: DrawnViewport,
    dots: boolean
  ) {
    const ctx = canvas?.getContext('2d');
    if (!ctx) return;
    ctx.fillStyle = cssVar('--canvas', '#f8fafc');
    ctx.fillRect(0, 0, width, height);
    ctx.strokeStyle = cssVar('--accent', '#0f766e');
    ctx.lineWidth = Math.max(1, dpr);
    const mid = height / 2;
    const scale = height * 0.44 * vp.vLo;
    const n = samples.length;
    ctx.beginPath();
    for (let i = 0; i < n; i += 1) {
      const x = (i / Math.max(1, n - 1)) * width;
      const y = mid - samples[i] * scale;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    ctx.stroke();
    if (dots) {
      ctx.fillStyle = cssVar('--accent', '#0f766e');
      const r = Math.max(2, dpr * 1.6);
      for (let i = 0; i < n; i += 1) {
        const x = (i / Math.max(1, n - 1)) * width;
        const y = mid - samples[i] * scale;
        ctx.beginPath();
        ctx.arc(x, y, r, 0, Math.PI * 2);
        ctx.fill();
      }
    }
    drawCursor2d(ctx, width, height, vp);
  }

  function drawWebglRaw(
    width: number,
    height: number,
    samples: Float32Array,
    vp: DrawnViewport,
    dots: boolean
  ) {
    const gl = canvas?.getContext('webgl2', { antialias: false, preserveDrawingBuffer: true });
    if (!gl) throw new Error('WebGL2 unavailable');
    const vertex = `#version 300 es
      in float a_time;
      in float a_amp;
      uniform float u_t0;
      uniform float u_t1;
      uniform float u_amp_scale;
      uniform float u_point;
      void main() {
        float x = ((a_time - u_t0) / (u_t1 - u_t0)) * 2.0 - 1.0;
        float y = clamp(a_amp * u_amp_scale, -1.0, 1.0);
        gl_Position = vec4(x, y, 0.0, 1.0);
        gl_PointSize = u_point;
      }`;
    const fragment = `#version 300 es
      precision mediump float;
      uniform vec3 u_color;
      out vec4 out_color;
      void main() {
        out_color = vec4(u_color, 1.0);
      }`;
    const program = makeProgram(gl, vertex, fragment);
    const n = samples.length;
    const vertices = new Float32Array(n * 2);
    for (let i = 0; i < n; i += 1) {
      const time = vp.t0 + (n <= 1 ? 0 : i / (n - 1)) * (vp.t1 - vp.t0);
      vertices[i * 2] = time;
      vertices[i * 2 + 1] = samples[i];
    }
    const background = hexToRgb01(cssVar('--canvas', '#f8fafc'));
    const color = hexToRgb01(cssVar('--accent', '#0f766e'));
    gl.viewport(0, 0, width, height);
    gl.clearColor(background[0], background[1], background[2], 1);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.useProgram(program);
    const buffer = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
    gl.bufferData(gl.ARRAY_BUFFER, vertices, gl.STREAM_DRAW);
    const timeLoc = gl.getAttribLocation(program, 'a_time');
    const ampLoc = gl.getAttribLocation(program, 'a_amp');
    gl.enableVertexAttribArray(timeLoc);
    gl.enableVertexAttribArray(ampLoc);
    gl.vertexAttribPointer(timeLoc, 1, gl.FLOAT, false, 8, 0);
    gl.vertexAttribPointer(ampLoc, 1, gl.FLOAT, false, 8, 4);
    gl.uniform1f(gl.getUniformLocation(program, 'u_t0'), vp.t0);
    gl.uniform1f(gl.getUniformLocation(program, 'u_t1'), vp.t1);
    gl.uniform1f(gl.getUniformLocation(program, 'u_amp_scale'), vp.vLo);
    gl.uniform3f(gl.getUniformLocation(program, 'u_color'), color[0], color[1], color[2]);
    const pointLoc = gl.getUniformLocation(program, 'u_point');
    gl.uniform1f(pointLoc, 0);
    gl.drawArrays(gl.LINE_STRIP, 0, n);
    if (dots) {
      gl.uniform1f(pointLoc, Math.max(3, (window.devicePixelRatio || 1) * 3));
      gl.drawArrays(gl.POINTS, 0, n);
    }
    gl.deleteBuffer(buffer);
    gl.deleteProgram(program);
  }
</script>

<section class="pane">
  <div class="pane-label">Waveform</div>
  {#if notice}
    <div class="notice">{notice}</div>
  {/if}
  {#key usingCanvas2d}
    <canvas
      bind:this={canvas}
      class="canvas"
      data-testid="waveform-canvas"
      data-render-token={renderToken}
      data-lod={lodMode}
      data-draw-generation="0"
      data-draw-time="0"
    ></canvas>
  {/key}
  {#if audio && onSelectionChange}
    <SelectionLayer
      {viewport}
      mode="time"
      {selection}
      onChange={onSelectionChange}
      {onSeek}
      {onDoubleZoom}
    />
  {/if}
  {#if audio && onScaleAmp}
    <div
      class="amp-gutter"
      data-testid="amp-gutter"
      role="slider"
      aria-label="Waveform amplitude scale"
      aria-valuenow={Math.round(viewport.ampScale * 100)}
      tabindex="-1"
      onpointerdown={ampPointerDown}
      onpointermove={ampPointerMove}
      onpointerup={ampPointerUp}
    ></div>
    {#if ampScaled}
      <button
        type="button"
        class="amp-reset"
        data-testid="amp-reset"
        title="Reset amplitude scale"
        onclick={() => onResetAmp?.()}
      >
        <IconRotateCcw aria-hidden="true" />
        <span>Reset</span>
      </button>
    {/if}
  {/if}
</section>

<style>
  .pane {
    position: relative;
    min-height: 11rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--canvas);
    overflow: hidden;
  }

  .canvas {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 11rem;
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

  .amp-gutter {
    position: absolute;
    top: 0;
    bottom: 0;
    left: 0;
    width: 1.1rem;
    z-index: 4;
    cursor: ns-resize;
    touch-action: none;
  }

  .amp-reset {
    position: absolute;
    bottom: 0.4rem;
    left: 1.5rem;
    z-index: 5;
    display: inline-flex;
    align-items: center;
    gap: 0.2rem;
    border: 1px solid color-mix(in oklab, var(--accent) 45%, var(--chrome-strong));
    border-radius: var(--radius-sm);
    background: var(--chip-bg);
    color: var(--accent-strong);
    padding: 0.12rem 0.34rem;
    font-size: 0.66rem;
    cursor: pointer;
    box-shadow: var(--shadow-sm);
  }

  .amp-reset :global(svg) {
    font-size: 0.72rem;
  }

  .amp-reset:hover {
    background: var(--accent-tint);
  }
</style>
