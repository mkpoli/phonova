<script lang="ts">
  import type { AudioInfo, CoreClientLike, ViewportState } from './types';
  import { applyCanvasSize, cssVar, measureCanvasTarget } from './rendering';

  interface Props {
    client: CoreClientLike | null;
    audio: AudioInfo | null;
    viewport: ViewportState;
    theme: 'light' | 'dark';
  }

  let { client, audio, viewport, theme }: Props = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  const cache = new Map<string, Float32Array>();
  const tileSeconds = 2;

  // The waveform reads as a ghost over the spectrogram: a translucent envelope
  // band with a dark halo outline, bright enough to trace the shape yet faint
  // enough that the palette stays the primary signal. Frozen per the RX-style
  // overlay convention in DESIGN.md.
  const GHOST_FILL_ALPHA = 0.22;
  const GHOST_STROKE_ALPHA = 0.7;
  const GHOST_HALO = 'rgba(4, 8, 16, 0.55)';

  let scheduled = false;

  function scheduleDraw() {
    if (scheduled) return;
    scheduled = true;
    requestAnimationFrame(() => {
      scheduled = false;
      void draw();
    });
  }

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
    viewport.ampScale;
    theme;
    scheduleDraw();
  });

  async function getSlice(width: number) {
    if (!client || !audio) return null;
    const px = Math.max(16, Math.floor(width / Math.max(1, window.devicePixelRatio || 1)));
    const tile0 = Math.floor(viewport.t0 / tileSeconds);
    const tile1 = Math.floor(viewport.t1 / tileSeconds);
    const key = `${String(audio.id)}:ghost:${tile0}:${tile1}:${px}`;
    const cached = cache.get(key);
    if (cached) return cached;
    const slice = await client.waveformSlice(audio.id, viewport.t0, viewport.t1, px);
    cache.set(key, slice.data);
    return slice.data;
  }

  async function draw() {
    if (!canvas) return;
    const { width, height, dpr } = measureCanvasTarget(canvas);
    if (!audio) {
      applyCanvasSize(canvas, width, height);
      canvas.getContext('2d')?.clearRect(0, 0, width, height);
      return;
    }
    const data = await getSlice(width);
    if (!data || !canvas) return;
    applyCanvasSize(canvas, width, height);
    const ctx = canvas.getContext('2d');
    if (!ctx) return;
    ctx.clearRect(0, 0, width, height);
    const mid = height / 2;
    const scale = height * 0.42 * viewport.ampScale;
    const buckets = data.length / 2;
    const stroke = cssVar('--overlay-ghost', '#e8e6df');

    // A closed band: the max envelope left to right, then the min envelope back.
    ctx.beginPath();
    for (let i = 0; i < buckets; i += 1) {
      const x = (i / Math.max(1, buckets - 1)) * width;
      const y = mid - data[i * 2 + 1] * scale;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    }
    for (let i = buckets - 1; i >= 0; i -= 1) {
      const x = (i / Math.max(1, buckets - 1)) * width;
      ctx.lineTo(x, mid - data[i * 2] * scale);
    }
    ctx.closePath();

    ctx.globalAlpha = GHOST_FILL_ALPHA;
    ctx.fillStyle = stroke;
    ctx.fill();

    // A dark halo under a lighter outline keeps the trace legible over any
    // palette in either theme.
    ctx.globalAlpha = 1;
    ctx.lineJoin = 'round';
    ctx.strokeStyle = GHOST_HALO;
    ctx.lineWidth = Math.max(1, dpr) * 2.4;
    ctx.stroke();
    ctx.globalAlpha = GHOST_STROKE_ALPHA;
    ctx.strokeStyle = stroke;
    ctx.lineWidth = Math.max(1, dpr);
    ctx.stroke();
    ctx.globalAlpha = 1;
  }
</script>

<canvas bind:this={canvas} class="ghost" data-testid="ghost-waveform" aria-hidden="true"></canvas>

<style>
  .ghost {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
    z-index: 1;
  }
</style>
