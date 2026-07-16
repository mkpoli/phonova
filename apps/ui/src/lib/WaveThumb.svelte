<script lang="ts">
  import type { AudioId, CoreClientLike } from './types';

  interface Props {
    client: CoreClientLike | null;
    audioId: AudioId | null;
    duration: number;
    theme: 'light' | 'dark';
    width?: number;
    height?: number;
  }

  let { client, audioId, duration, theme, width = 132, height = 40 }: Props = $props();

  let canvas = $state<HTMLCanvasElement | null>(null);
  let ready = $state(false);

  $effect(() => {
    const node = canvas;
    const id = audioId;
    // Re-read theme so a palette flip repaints from the cached slice.
    void theme;
    if (!node || !client || id === null || duration <= 0) return;
    let cancelled = false;
    const dpr = Math.min(2, window.devicePixelRatio || 1);
    node.width = Math.round(width * dpr);
    node.height = Math.round(height * dpr);
    const px = Math.max(32, Math.round(width));
    client
      .waveformSlice(id, 0, duration, px)
      .then((slice) => {
        if (cancelled) return;
        paint(node, slice.data, dpr);
        ready = true;
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  });

  function paint(node: HTMLCanvasElement, data: Float32Array, dpr: number) {
    const ctx = node.getContext('2d');
    if (!ctx) return;
    const w = node.width;
    const h = node.height;
    ctx.clearRect(0, 0, w, h);
    const style = getComputedStyle(node);
    const stroke = style.getPropertyValue('--accent').trim() || '#0f766e';
    const mid = h / 2;
    const columns = Math.floor(data.length / 2);
    const step = columns > 0 ? w / columns : w;
    ctx.fillStyle = stroke;
    for (let i = 0; i < columns; i += 1) {
      const min = data[i * 2];
      const max = data[i * 2 + 1];
      const top = mid - max * mid * 0.92;
      const bottom = mid - min * mid * 0.92;
      const x = i * step;
      ctx.fillRect(x, top, Math.max(dpr, step), Math.max(dpr, bottom - top));
    }
  }
</script>

<canvas
  bind:this={canvas}
  class="thumb"
  class:ready
  style:width={`${width}px`}
  style:height={`${height}px`}
  data-testid="wave-thumb"
  aria-hidden="true"
></canvas>

<style>
  .thumb {
    display: block;
    border-radius: 4px;
    background: var(--canvas);
    opacity: 0;
    transition: opacity 0.18s ease;
  }

  .thumb.ready {
    opacity: 1;
  }
</style>
