<script lang="ts">
  import type {
    AudioInfo,
    CoreClientLike,
    OverlayParams,
    OverlayStats,
    SpectrogramTileRequest,
    ViewportState,
    WasmColormapName
  } from './types';
  import { cssVar, FrameTimeMonitor, hexToRgb01, makeProgram, resizeCanvas } from './rendering';
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
  }

  let { client, audio, viewport, cursorTime, theme, colormap, overlayParams, onOverlayStats }: Props =
    $props();
  let canvas = $state<HTMLCanvasElement | null>(null);
  let notice = $state('');
  let usingCanvas2d = $state(false);
  let renderToken = $state(0);

  const cache = new Map<string, ImageBitmap>();
  const monitor = new FrameTimeMonitor();
  const tileSeconds = 2;

  $effect(() => {
    if (!canvas) return;
    const observer = new ResizeObserver(() => scheduleDraw());
    observer.observe(canvas);
    scheduleDraw();
    return () => observer.disconnect();
  });

  $effect(() => {
    audio?.id;
    viewport.t0;
    viewport.t1;
    viewport.f0;
    viewport.f1;
    cursorTime;
    theme;
    colormap;
    scheduleDraw();
  });

  function scheduleDraw() {
    requestAnimationFrame(() => void draw());
  }

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

  async function draw() {
    if (!canvas) return;
    const start = performance.now();
    const { width, height, dpr } = resizeCanvas(canvas);
    const bitmap = await getTile(width, height);
    if (!bitmap) {
      drawEmpty(width, height);
      return;
    }
    if (usingCanvas2d) {
      drawCanvas2d(width, height, dpr, bitmap);
    } else {
      try {
        drawWebgl(width, height, bitmap);
      } catch {
        usingCanvas2d = true;
        notice = 'Canvas fallback active';
        drawCanvas2d(width, height, dpr, bitmap);
      }
    }
    const elapsed = performance.now() - start;
    if (!usingCanvas2d && monitor.record(elapsed)) {
      usingCanvas2d = true;
      notice = 'Canvas fallback active';
    }
    renderToken += 1;
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
    <canvas bind:this={canvas} class="canvas" data-testid="spectrogram-canvas" data-render-token={renderToken}></canvas>
  {/key}
  <TrackOverlay {client} {audio} {viewport} {theme} params={overlayParams} onStats={onOverlayStats} />
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
  }

  .canvas {
    display: block;
    width: 100%;
    height: 100%;
    min-height: 16rem;
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
