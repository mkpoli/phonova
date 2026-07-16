import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openEditorWithFixture } from './helpers';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const shortFixture = path.join(root, 'tests/fixtures/audio/arctic_bdl_a0001.wav');
const screenshots = path.join(here, 'screenshots');

const NEW_PALETTES = ['Inferno', 'Plasma', 'Cividis'] as const;

/** Reads a numeric data attribute off a canvas by test id. */
async function attr(page: Page, testId: string, name: string) {
  return page.getByTestId(testId).evaluate((node, n) => Number(node.getAttribute(n)), name);
}

async function dispatchWheel(page: Page, deltaY: number, modifiers: { shiftKey?: boolean } = {}) {
  await page.getByTestId('timeline').evaluate(
    (node, { deltaY, modifiers }) => {
      const rect = node.getBoundingClientRect();
      node.dispatchEvent(
        new WheelEvent('wheel', {
          bubbles: true,
          cancelable: true,
          deltaY,
          clientX: rect.left + rect.width / 2,
          clientY: rect.top + rect.height / 2,
          ...modifiers
        })
      );
    },
    { deltaY, modifiers }
  );
}

test('pan moves waveform, spectrogram, and overlays as one rigid sheet', async ({ page }) => {
  await openEditorWithFixture(page, shortFixture);
  await expect(page.getByTestId('waveform-canvas')).toHaveAttribute('data-render-token', /[1-9]/);
  await expect(page.getByTestId('spectrogram-canvas')).toHaveAttribute('data-render-token', /[1-9]/);

  // Zoom in so a pan actually moves (a fully zoomed-out view clamps scrolling).
  await page.getByTestId('timeline').hover();
  const fullSpan = await page.getByTestId('editor').evaluate(
    (n) =>
      Number(n.getAttribute('data-visible-end')) - Number(n.getAttribute('data-visible-start'))
  );
  await dispatchWheel(page, -500);
  await dispatchWheel(page, -500);
  await expect
    .poll(() =>
      page.getByTestId('editor').evaluate(
        (n) =>
          Number(n.getAttribute('data-visible-end')) -
          Number(n.getAttribute('data-visible-start'))
      )
    )
    .toBeLessThan(fullSpan);
  await page.waitForTimeout(200);

  // A pan must be reflected on the waveform and spectrogram within a frame: the
  // panes redraw their existing pixels with a CSS transform instantly, without
  // waiting on a worker round-trip. Assert the draw generation advances well
  // inside the frame budget, and the rendered element visibly moves.
  const before = await page.getByTestId('spectrogram-canvas').screenshot();
  const waveGen0 = await attr(page, 'waveform-canvas', 'data-draw-generation');
  const specGen0 = await attr(page, 'spectrogram-canvas', 'data-draw-generation');
  const overlayTok0 = await attr(page, 'track-overlay', 'data-overlay-token');

  const started = Date.now();
  await dispatchWheel(page, 200, { shiftKey: true });

  await expect
    .poll(() => attr(page, 'spectrogram-canvas', 'data-draw-generation'), { timeout: 100 })
    .toBeGreaterThan(specGen0);
  const specLatency = Date.now() - started;
  expect(specLatency).toBeLessThan(100);

  await expect
    .poll(() => attr(page, 'waveform-canvas', 'data-draw-generation'))
    .toBeGreaterThan(waveGen0);
  await expect
    .poll(() => attr(page, 'track-overlay', 'data-overlay-token'))
    .toBeGreaterThan(overlayTok0);

  // The waveform and spectrogram remap in the same effect flush, so their draw
  // timestamps sit within one frame of each other — no visible desync.
  const waveTime = await attr(page, 'waveform-canvas', 'data-draw-time');
  const specTime = await attr(page, 'spectrogram-canvas', 'data-draw-time');
  expect(Math.abs(waveTime - specTime)).toBeLessThan(16);

  // The rendered spectrogram visibly moved (the CSS transform is composited).
  const after = await page.getByTestId('spectrogram-canvas').screenshot();
  expect(Buffer.compare(before, after)).not.toBe(0);
});

test('palette switch recolorizes under 300 ms and never recomputes the STFT', async ({ page }) => {
  await openEditorWithFixture(page, shortFixture);
  await expect(page.getByTestId('spectrogram-canvas')).toHaveAttribute('data-render-token', /[1-9]/);

  // Isolated engine timing from the worker perf probe: the first colorize warms
  // the raw-dB block cache (STFT + colorize), the second re-colorizes the same
  // cached dB under a different palette. The block count must not grow.
  const probe = await page.evaluate(async () => {
    const hook = (globalThis as { __phonix?: { client?: unknown; audioId?: number | null } })
      .__phonix;
    const client = hook?.client as {
      spectrogramProbe: (
        id: number,
        req: Record<string, unknown>
      ) => Promise<{
        stftMs: number;
        recolorMs: number;
        blocksAfterStft: number;
        blocksAfterRecolor: number;
      }>;
    };
    const editor = document.querySelector('[data-testid="editor"]');
    const t0 = Number(editor?.getAttribute('data-visible-start'));
    const t1 = Number(editor?.getAttribute('data-visible-end'));
    return client.spectrogramProbe(hook!.audioId as number, {
      // The whole visible range: the cold pass computes every frame column's
      // STFT across the covered blocks, so its cost dominates a fixed-size
      // colorize and the recolor speedup is measurable.
      t0,
      t1,
      f0: 0,
      f1: 5000,
      widthPx: 700,
      heightPx: 256,
      // A non-default window keys blocks the panes never rendered, so the first
      // colorize is a genuine cold STFT rather than a cache hit.
      windowLength: 0.006,
      maxFrequency: 5000,
      timeStep: 0.002,
      frequencyStep: 20,
      dynamicRangeDb: 70,
      colormap: 'Viridis',
      theme: 'Dark'
    });
  });
  console.log(
    `spectrogram-probe stft=${probe.stftMs.toFixed(2)}ms recolor=${probe.recolorMs.toFixed(2)}ms ` +
      `blocks ${probe.blocksAfterStft}->${probe.blocksAfterRecolor}`
  );
  expect(probe.blocksAfterRecolor).toBe(probe.blocksAfterStft);
  expect(probe.recolorMs).toBeLessThan(probe.stftMs);

  // End-to-end: switching the palette in the transport repaints the spectrogram
  // visibly within 300 ms (the cached dB is only re-colorized).
  const tokenBefore = await attr(page, 'spectrogram-canvas', 'data-render-token');
  const started = Date.now();
  await page.getByLabel('Spectrogram palette').selectOption('Inferno');
  await expect
    .poll(() => attr(page, 'spectrogram-canvas', 'data-render-token'), { timeout: 5000 })
    .toBeGreaterThan(tokenBefore);
  const elapsed = Date.now() - started;
  console.log(`palette-switch repaint ${elapsed} ms`);
  expect(elapsed).toBeLessThan(300);
});

test('new palettes render legibly in both themes', async ({ page }) => {
  await openEditorWithFixture(page, shortFixture);
  await expect(page.getByTestId('spectrogram-canvas')).toHaveAttribute('data-render-token', /[1-9]/);

  for (const palette of NEW_PALETTES) {
    const tokenBefore = await attr(page, 'spectrogram-canvas', 'data-render-token');
    await page.getByLabel('Spectrogram palette').selectOption(palette);
    await expect
      .poll(() => attr(page, 'spectrogram-canvas', 'data-render-token'))
      .toBeGreaterThan(tokenBefore);
    // The tile carries many distinct colors, not a flat fill.
    const unique = await page.getByTestId('spectrogram-canvas').evaluate(async (canvas) => {
      const bitmap = await createImageBitmap(canvas as HTMLCanvasElement);
      const off = new OffscreenCanvas(Math.min(96, bitmap.width), Math.min(96, bitmap.height));
      const ctx = off.getContext('2d');
      if (!ctx) return 0;
      ctx.drawImage(bitmap, 0, 0, off.width, off.height);
      const pixels = ctx.getImageData(0, 0, off.width, off.height).data;
      const colors = new Set<string>();
      for (let i = 0; i < pixels.length; i += 16) {
        colors.add(`${pixels[i]},${pixels[i + 1]},${pixels[i + 2]}`);
      }
      return colors.size;
    });
    expect(unique).toBeGreaterThan(8);

    await page.screenshot({
      path: path.join(screenshots, `palette-${palette.toLowerCase()}-light.png`),
      fullPage: true
    });
    await page.getByLabel('Toggle theme').click();
    await expect(page.locator('html')).toHaveClass(/dark/);
    await page.waitForTimeout(200);
    await page.screenshot({
      path: path.join(screenshots, `palette-${palette.toLowerCase()}-dark.png`),
      fullPage: true
    });
    await page.getByLabel('Toggle theme').click();
    await expect(page.locator('html')).not.toHaveClass(/dark/);
  }
});
