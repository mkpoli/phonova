<script lang="ts">
  interface Props {
    leftPct: number;
    active: boolean;
    linked: boolean;
    dragging: boolean;
    onGrab: (clientX: number) => void;
    onDrag: (clientX: number) => void;
    onRelease: (clientX: number) => void;
  }

  let { leftPct, active, linked, dragging, onGrab, onDrag, onRelease }: Props = $props();

  function handlePointerDown(event: PointerEvent) {
    if (event.button !== 0) return;
    event.preventDefault();
    event.stopPropagation();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    onGrab(event.clientX);
  }

  function handlePointerMove(event: PointerEvent) {
    if (!dragging) return;
    onDrag(event.clientX);
  }

  function handlePointerUp(event: PointerEvent) {
    if (!dragging) return;
    const el = event.currentTarget as HTMLElement;
    if (el.hasPointerCapture(event.pointerId)) el.releasePointerCapture(event.pointerId);
    onRelease(event.clientX);
  }
</script>

<div
  class="handle"
  class:active
  class:linked
  class:dragging
  data-testid="boundary-handle"
  style={`left:${leftPct}%`}
  role="separator"
  aria-orientation="vertical"
  onpointerdown={handlePointerDown}
  onpointermove={handlePointerMove}
  onpointerup={handlePointerUp}
>
  <span class="line"></span>
</div>

<style>
  .handle {
    position: absolute;
    top: 0;
    bottom: 0;
    width: 11px;
    margin-left: -5px;
    display: flex;
    justify-content: center;
    cursor: ew-resize;
    z-index: 3;
    touch-action: none;
  }

  .line {
    width: 1px;
    height: 100%;
    background: var(--boundary, #64748b);
  }

  .handle:hover .line,
  .handle.active .line {
    width: 2px;
    background: var(--accent);
  }

  .handle.linked .line {
    background: var(--warn);
  }

  .handle.dragging .line {
    width: 2px;
    background: var(--accent-strong);
  }
</style>
