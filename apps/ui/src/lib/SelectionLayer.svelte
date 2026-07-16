<script lang="ts">
  import type { Selection, SelectionMode, ViewportState } from './types';

  interface Props {
    viewport: ViewportState;
    mode: SelectionMode;
    selection: Selection | null;
    onChange: (selection: Selection | null) => void;
    onSeek?: (time: number) => void;
  }

  let { viewport, mode, selection, onChange, onSeek }: Props = $props();

  let root = $state<HTMLDivElement | null>(null);
  let dragging = $state(false);
  // Pixel origin of the drag, to tell a click apart from a box.
  let startX = 0;
  let startY = 0;
  let startT = 0;
  let startF = 0;

  const CLICK_SLOP_PX = 3;

  function ratios(event: PointerEvent) {
    const rect = root!.getBoundingClientRect();
    const rx = Math.min(1, Math.max(0, (event.clientX - rect.left) / rect.width));
    const ry = Math.min(1, Math.max(0, (event.clientY - rect.top) / rect.height));
    return { rx, ry };
  }

  function timeAt(rx: number) {
    return viewport.t0 + rx * (viewport.t1 - viewport.t0);
  }

  function freqAt(ry: number) {
    return viewport.f1 - ry * (viewport.f1 - viewport.f0);
  }

  function build(curT: number, curF: number): Selection {
    if (mode === 'time') {
      return {
        t0: Math.min(startT, curT),
        t1: Math.max(startT, curT),
        f0: viewport.f0,
        f1: viewport.f1,
        mode
      };
    }
    return {
      t0: Math.min(startT, curT),
      t1: Math.max(startT, curT),
      f0: Math.min(startF, curF),
      f1: Math.max(startF, curF),
      mode
    };
  }

  function onPointerDown(event: PointerEvent) {
    if (event.button !== 0 || !root) return;
    // Own the gesture: the timeline's click-to-seek must not also fire.
    event.stopPropagation();
    const { rx, ry } = ratios(event);
    startX = event.clientX;
    startY = event.clientY;
    startT = timeAt(rx);
    startF = freqAt(ry);
    dragging = true;
    root.setPointerCapture(event.pointerId);
  }

  function onPointerMove(event: PointerEvent) {
    if (!dragging) return;
    event.stopPropagation();
    const { rx, ry } = ratios(event);
    onChange(build(timeAt(rx), freqAt(ry)));
  }

  function onPointerUp(event: PointerEvent) {
    if (!dragging) return;
    event.stopPropagation();
    dragging = false;
    root?.releasePointerCapture(event.pointerId);
    const movedX = Math.abs(event.clientX - startX);
    const movedY = Math.abs(event.clientY - startY);
    const isClick = movedX < CLICK_SLOP_PX && (mode === 'time' || movedY < CLICK_SLOP_PX);
    if (isClick) {
      onChange(null);
      onSeek?.(startT);
      return;
    }
    const { rx, ry } = ratios(event);
    onChange(build(timeAt(rx), freqAt(ry)));
  }

  // Selection geometry mapped to this pane's pixels, or null when the box has
  // slid entirely out of view.
  const rect = $derived.by(() => {
    if (!selection) return null;
    const span = viewport.t1 - viewport.t0;
    const left = ((selection.t0 - viewport.t0) / span) * 100;
    const right = ((selection.t1 - viewport.t0) / span) * 100;
    if (right < 0 || left > 100) return null;
    let top = 0;
    let bottom = 100;
    if (mode === 'box') {
      const fspan = Math.max(1, viewport.f1 - viewport.f0);
      top = (1 - (selection.f1 - viewport.f0) / fspan) * 100;
      bottom = (1 - (selection.f0 - viewport.f0) / fspan) * 100;
    }
    return {
      left: Math.max(0, left),
      width: Math.min(100, right) - Math.max(0, left),
      top: Math.max(0, top),
      height: Math.min(100, bottom) - Math.max(0, top)
    };
  });
</script>

<div
  bind:this={root}
  class="layer"
  role="application"
  aria-label={mode === 'box' ? 'Spectrogram selection' : 'Waveform selection'}
  data-testid="selection-layer-{mode}"
  onpointerdown={onPointerDown}
  onpointermove={onPointerMove}
  onpointerup={onPointerUp}
>
  {#if rect}
    <div
      class="box"
      data-testid="selection-box"
      data-sel-mode={mode}
      data-sel-t0={selection?.t0}
      data-sel-t1={selection?.t1}
      data-sel-f0={selection?.f0}
      data-sel-f1={selection?.f1}
      style="left:{rect.left}%; width:{rect.width}%; top:{rect.top}%; height:{rect.height}%;"
    ></div>
  {/if}
</div>

<style>
  .layer {
    position: absolute;
    inset: 0;
    z-index: 3;
    cursor: crosshair;
    touch-action: none;
  }

  .box {
    position: absolute;
    box-sizing: border-box;
    border: 1px solid var(--accent, #0f766e);
    background: color-mix(in oklab, var(--accent, #0f766e) 20%, transparent);
    box-shadow: 0 0 0 1px color-mix(in oklab, var(--accent, #0f766e) 40%, transparent);
    pointer-events: none;
  }
</style>
