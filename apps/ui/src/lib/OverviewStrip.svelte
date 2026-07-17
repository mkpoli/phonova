<script lang="ts">
  import type { AudioInfo, CoreClientLike, ViewportState } from './types';
  import { clampViewport } from './types';
  import { applyCanvasSize, cssVar, measureCanvasTarget } from './rendering';

  interface Props {
    client: CoreClientLike | null;
    audio: AudioInfo | null;
    viewport: ViewportState;
    theme: 'light' | 'dark';
    onViewportChange: (viewport: ViewportState) => void;
  }

  let { client, audio, viewport, theme, onViewportChange }: Props = $props();
  let canvas = $state<HTMLCanvasElement | null>(null);
  let dragOffset = $state<number | null>(null);

  const cache = new Map<string, Float32Array>();

  $effect(() => {
    if (!canvas) return;
    const observer = new ResizeObserver(() => draw());
    observer.observe(canvas);
    draw();
    return () => observer.disconnect();
  });

  $effect(() => {
    audio?.id;
    viewport.t0;
    viewport.t1;
    theme;
    draw();
  });

  async function getSlice(width: number) {
    if (!client || !audio) return null;
    const px = Math.max(8, Math.floor(width / Math.max(1, window.devicePixelRatio || 1)));
    const key = `${String(audio.id)}:overview:${px}`;
    const cached = cache.get(key);
    if (cached) return cached;
    const slice = await client.waveformSlice(audio.id, 0, audio.duration, px);
    cache.set(key, slice.data);
    return slice.data;
  }

  async function draw() {
    if (!canvas) return;
    // Measure without resizing the backing store yet: the old bitmap keeps
    // showing, stretched to the new CSS box by the browser, while this awaits
    // a fresh slice — a resize never clears the strip to blank first.
    const { width, height, dpr } = measureCanvasTarget(canvas);
    if (!audio) {
      applyCanvasSize(canvas, width, height);
      const ctx = canvas.getContext('2d');
      if (!ctx) return;
      ctx.fillStyle = cssVar('--canvas', '#f8fafc');
      ctx.fillRect(0, 0, width, height);
      return;
    }
    const data = await getSlice(width);
    if (!data || !canvas) return;
    applyCanvasSize(canvas, width, height);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, width, height);
    ctx.fillStyle = cssVar('--canvas', '#f8fafc');
    ctx.fillRect(0, 0, width, height);
    ctx.strokeStyle = cssVar('--accent', '#0f766e');
    ctx.lineWidth = Math.max(1, dpr);
    ctx.beginPath();
    const mid = height / 2;
    const scale = height * 0.42;
    const buckets = data.length / 2;
    for (let i = 0; i < buckets; i += 1) {
      const x = (i / Math.max(1, buckets - 1)) * width;
      ctx.moveTo(x, mid - data[i * 2 + 1] * scale);
      ctx.lineTo(x, mid - data[i * 2] * scale);
    }
    ctx.stroke();
    const left = (viewport.t0 / audio.duration) * width;
    const right = (viewport.t1 / audio.duration) * width;
    ctx.fillStyle = 'rgba(20, 184, 166, 0.16)';
    ctx.strokeStyle = cssVar('--accent-strong', '#115e59');
    ctx.fillRect(left, 0, Math.max(2, right - left), height);
    ctx.strokeRect(left, 0.5, Math.max(2, right - left), height - 1);
  }

  function seekFromPointer(event: PointerEvent) {
    if (!canvas || !audio) return;
    const rect = canvas.getBoundingClientRect();
    const ratio = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    const span = viewport.t1 - viewport.t0;
    const center = ratio * audio.duration;
    const offset = dragOffset ?? span / 2;
    onViewportChange(clampViewport({ ...viewport, t0: center - offset, t1: center - offset + span }, audio.duration));
  }

  function pointerDown(event: PointerEvent) {
    if (!canvas || !audio) return;
    const rect = canvas.getBoundingClientRect();
    const at = ((event.clientX - rect.left) / rect.width) * audio.duration;
    dragOffset = at >= viewport.t0 && at <= viewport.t1 ? at - viewport.t0 : null;
    canvas.setPointerCapture(event.pointerId);
    seekFromPointer(event);
  }
</script>

<canvas
  bind:this={canvas}
  class="overview"
  data-testid="overview-canvas"
  onpointerdown={pointerDown}
  onpointermove={(event) => {
    if (event.buttons === 1) seekFromPointer(event);
  }}
  onpointerup={(event) => {
    dragOffset = null;
    event.currentTarget.releasePointerCapture(event.pointerId);
  }}
></canvas>

<style>
  .overview {
    display: block;
    width: 100%;
    height: 4rem;
    border-bottom: 1px solid var(--chrome-strong);
    background: var(--canvas);
    touch-action: none;
  }
</style>
