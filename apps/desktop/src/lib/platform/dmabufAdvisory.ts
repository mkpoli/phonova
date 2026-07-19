import { invoke } from '@tauri-apps/api/core';

/** `localStorage` key marking the advisory already shown, so it never repeats. */
const ADVISED_KEY = 'phonix:dmabuf-advised';
/**
 * Average frame time, in ms, above which the probe calls rendering slow.
 * Mirrors the render panes' own Canvas2D fallback threshold
 * (`FRAME_FALLBACK_MS` in `@phonia/ui`'s rendering module) without importing
 * it, since that module is internal to the shared UI package.
 */
const SLOW_FRAME_MS = 32;
/** Probe window length. Long enough to outlast a first-frame warm-up hiccup. */
const PROBE_FRAMES = 20;

interface DmabufAdvisory {
  linux: boolean;
  envSet: boolean;
}

/**
 * Draws a few frames on a small canvas attached to the document (so it goes
 * through the real compositor path, unlike an `OffscreenCanvas`) and returns
 * the average time between frames, in ms. Resolves `Infinity` when WebGL2 is
 * unavailable at all, since Canvas2D is the only option either way.
 */
function probeFramePace(): Promise<number> {
  return new Promise((resolve) => {
    const canvas = document.createElement('canvas');
    canvas.width = 64;
    canvas.height = 64;
    canvas.style.cssText = 'position:fixed;left:-9999px;top:-9999px;';
    document.body.appendChild(canvas);
    const gl = canvas.getContext('webgl2');
    if (!gl) {
      canvas.remove();
      resolve(Infinity);
      return;
    }
    let frame = 0;
    let last = performance.now();
    const deltas: number[] = [];
    const tick = () => {
      const now = performance.now();
      if (frame > 0) deltas.push(now - last);
      last = now;
      gl.clearColor(frame % 2, 0, 0, 1);
      gl.clear(gl.COLOR_BUFFER_BIT);
      frame += 1;
      if (frame < PROBE_FRAMES) {
        requestAnimationFrame(tick);
        return;
      }
      canvas.remove();
      resolve(deltas.reduce((sum, value) => sum + value, 0) / deltas.length);
    };
    requestAnimationFrame(tick);
  });
}

/**
 * Makes every `webgl`/`webgl2` context request return `null`, from this point
 * on, for the life of the page.
 *
 * The waveform and spectrogram panes already catch a failed WebGL2 context
 * and drop to their own Canvas2D draw path; this reuses exactly that path
 * instead of duplicating it, by cutting off WebGL at its one entry point
 * (`HTMLCanvasElement.getContext`) rather than threading a flag through the
 * shared render panes.
 */
function forceCanvas2dFallback() {
  // `getContext` is heavily overloaded per context-id string; there is no
  // single typed signature that covers replacing it generically, so this
  // steps outside the type system at the one point that requires it.
  type AnyGetContext = (this: HTMLCanvasElement, ...args: unknown[]) => unknown;
  const proto = HTMLCanvasElement.prototype;
  const original = proto.getContext as unknown as AnyGetContext;
  const patched: AnyGetContext = function (...args) {
    if (args[0] === 'webgl' || args[0] === 'webgl2') return null;
    return original.apply(this, args);
  };
  proto.getContext = patched as unknown as typeof proto.getContext;
}

/**
 * Checks once whether this launch is likely hitting WebKitGTK's slow DMABUF
 * render path; if so, routes every canvas to the Canvas2D fallback and
 * returns an advisory message to show the user.
 *
 * Resolves `null` when there is nothing to do: not Linux, the
 * `WEBKIT_DISABLE_DMABUF_RENDERER` workaround is already exported, the
 * advisory already ran on a previous launch, or the probe came back fast.
 */
export async function applyDmabufGuard(): Promise<string | null> {
  let advisory: DmabufAdvisory;
  try {
    advisory = await invoke<DmabufAdvisory>('dmabuf_advisory');
  } catch {
    return null;
  }
  if (!advisory.linux || advisory.envSet) return null;
  if (localStorage.getItem(ADVISED_KEY)) return null;

  const avgFrameMs = await probeFramePace();
  if (avgFrameMs <= SLOW_FRAME_MS) return null;

  forceCanvas2dFallback();
  localStorage.setItem(ADVISED_KEY, '1');
  return (
    'Graphics rendering looks slow on this system, so Phonia switched to its Canvas2D ' +
    'renderer. If this is WebKitGTK’s DMABUF renderer, relaunch with ' +
    'WEBKIT_DISABLE_DMABUF_RENDERER=1 set to work around it.'
  );
}
