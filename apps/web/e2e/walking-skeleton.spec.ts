import { expect, test } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const shortFixture = path.join(root, 'tests/fixtures/audio/arctic_bdl_a0001.wav');
const longFixture = path.join(root, 'tests/fixtures/audio/long_scroll_test.wav');
const screenshots = path.join(here, 'screenshots');

test('drop wav, render, zoom, play, and scroll long fixture', async ({ page }) => {
  await page.goto('/');
  await page.getByTestId('file-input').setInputFiles(shortFixture);
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);
  await expect(page.getByTestId('waveform-canvas')).toHaveAttribute('data-render-token', /[1-9]/);
  await expect(page.getByTestId('spectrogram-canvas')).toHaveAttribute('data-render-token', /[1-9]/);

  const waveformVariation = await canvasVariation(page, 'waveform-canvas');
  const spectrogramVariation = await canvasVariation(page, 'spectrogram-canvas');
  expect(waveformVariation.uniqueColors).toBeGreaterThan(2);
  expect(spectrogramVariation.uniqueColors).toBeGreaterThan(8);

  const beforeZoom = await visibleSpan(page);
  await page.getByTestId('timeline').hover();
  await page.mouse.wheel(0, -500);
  await expect.poll(() => visibleSpan(page)).toBeLessThan(beforeZoom);
  const afterZoomIn = await visibleSpan(page);
  await page.mouse.wheel(0, 500);
  await expect.poll(() => visibleSpan(page)).toBeGreaterThan(afterZoomIn);

  const beforePlay = await cursorTime(page);
  await page.getByLabel('Play').click();
  await page.waitForTimeout(2100);
  const afterPlay = await cursorTime(page);
  expect(afterPlay - beforePlay).toBeGreaterThan(1.2);

  await page.screenshot({ path: path.join(screenshots, 'waveform-light.png'), fullPage: true });
  await page.getByLabel('Toggle theme').click();
  await expect(page.locator('html')).toHaveClass(/dark/);
  await page.screenshot({ path: path.join(screenshots, 'waveform-dark.png'), fullPage: true });

  await page.getByTestId('file-input').setInputFiles(longFixture);
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);
  const frameStats = await measureScrollFrames(page);
  console.log(
    `long-scroll-frame-ms p50=${frameStats.p50.toFixed(2)} p95=${frameStats.p95.toFixed(2)} max=${frameStats.max.toFixed(2)} samples=${frameStats.samples}`
  );
  expect(frameStats.p95).toBeLessThan(32);
});

test('analysis overlays: toggles, live pitch ceiling, clipping badge, screenshots', async ({
  page
}) => {
  test.setTimeout(180_000);
  await page.goto('/');
  await page.getByTestId('file-input').setInputFiles(shortFixture);
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);

  const overlay = page.getByTestId('track-overlay');
  // Wait for the whole-signal pitch track (the slow one) to settle.
  await expect
    .poll(() => pitchMax(page), { timeout: 60_000 })
    .toBeGreaterThan(0);

  // All three overlays draw marks over the spectrogram.
  const allOn = await overlayMarks(page);
  expect(allOn).toBeGreaterThan(200);

  // Hiding formants (the densest layer) drops the mark count; showing restores.
  await page.getByTestId('toggle-formant').uncheck();
  await expect.poll(() => overlayMarks(page)).toBeLessThan(allOn);
  const withoutFormants = await overlayMarks(page);
  await page.getByTestId('toggle-formant').check();
  await expect.poll(() => overlayMarks(page)).toBeGreaterThan(withoutFormants);

  // Hiding every track clears the overlay.
  await page.getByTestId('toggle-pitch').uncheck();
  await page.getByTestId('toggle-formant').uncheck();
  await page.getByTestId('toggle-intensity').uncheck();
  await expect.poll(() => overlayMarks(page)).toBe(0);
  await page.getByTestId('toggle-pitch').check();
  await page.getByTestId('toggle-formant').check();
  await page.getByTestId('toggle-intensity').check();
  await expect.poll(() => pitchMax(page), { timeout: 60_000 }).toBeGreaterThan(0);

  // Default ceiling (600 Hz) does not clip this male fixture.
  await expect(page.getByTestId('pitch-clip-badge')).toHaveCount(0);

  // Zoom in so the visible-span pitch preview covers a short window (a
  // word-level view, the scale at which pitch is actually inspected).
  await page.getByTestId('timeline').hover();
  for (let i = 0; i < 12; i += 1) {
    await page.mouse.wheel(0, -400);
    await page.waitForTimeout(80);
  }
  expect(await visibleSpan(page)).toBeLessThan(0.5);

  // Warm up: change the ceiling and wait for the whole-signal recompute to
  // settle, leaving the worker idle before the timed edit.
  const warmMax = await pitchMax(page);
  await page.getByTestId('pitch-ceiling').fill('500');
  await expect.poll(() => pitchMax(page), { timeout: 60_000 }).not.toBe(warmMax);

  // Timed edit: the visible span must re-render well under 500 ms.
  const beforeToken = await pitchDataToken(page);
  const started = Date.now();
  await page.getByTestId('pitch-ceiling').fill('400');
  await expect
    .poll(() => pitchDataToken(page), { timeout: 20_000 })
    .toBeGreaterThan(beforeToken);
  const elapsed = Date.now() - started;
  console.log(`pitch-ceiling visible-span re-render ${elapsed} ms`);
  expect(elapsed).toBeLessThan(500);

  // Lowering the ceiling into the tracked pitch raises the clipping badge.
  await page.getByTestId('pitch-ceiling').fill('500');
  await expect(page.getByTestId('pitch-clip-badge')).toBeVisible({ timeout: 60_000 });

  // Fresh screenshots with overlays on, in both themes. Fit the whole file so
  // the contours read across several vowels (400 Hz ceiling suits the male
  // fixture; the cached whole-signal tracks redraw without recomputing).
  await page.getByTestId('pitch-ceiling').fill('400');
  await page.getByTestId('spectrogram-canvas').click();
  await page.keyboard.press('0');
  await expect.poll(() => visibleSpan(page)).toBeGreaterThan(2);
  // Wait for the whole-signal track at this ceiling to settle, so the pitch
  // line spans the whole file rather than only the previously zoomed window.
  await expect
    .poll(() => pitchMax(page), { timeout: 60_000 })
    .toBeLessThan(420);
  await expect.poll(() => pitchMax(page)).toBeGreaterThan(0);
  await page.waitForTimeout(600);
  await page.screenshot({ path: path.join(screenshots, 'overlays-light.png'), fullPage: true });
  await page.getByLabel('Toggle theme').click();
  await expect(page.locator('html')).toHaveClass(/dark/);
  await page.waitForTimeout(600);
  await page.screenshot({ path: path.join(screenshots, 'overlays-dark.png'), fullPage: true });
});

function pitchMax(page: import('@playwright/test').Page) {
  return page
    .getByTestId('track-overlay')
    .evaluate((node) => Number(node.getAttribute('data-pitch-max')));
}

function pitchDataToken(page: import('@playwright/test').Page) {
  return page
    .getByTestId('track-overlay')
    .evaluate((node) => Number(node.getAttribute('data-pitch-data-token')));
}

async function overlayMarks(page: import('@playwright/test').Page) {
  return page.getByTestId('track-overlay').evaluate(async (canvas: HTMLCanvasElement) => {
    const bitmap = await createImageBitmap(canvas);
    const w = Math.min(240, bitmap.width);
    const h = Math.min(160, bitmap.height);
    const off = new OffscreenCanvas(w, h);
    const ctx = off.getContext('2d');
    if (!ctx) return 0;
    ctx.drawImage(bitmap, 0, 0, w, h);
    const pixels = ctx.getImageData(0, 0, w, h).data;
    let marks = 0;
    for (let i = 3; i < pixels.length; i += 4) {
      if (pixels[i] > 24) marks += 1;
    }
    return marks;
  });
}

async function visibleSpan(page: import('@playwright/test').Page) {
  return page.getByTestId('editor').evaluate((node) => {
    const start = Number(node.getAttribute('data-visible-start'));
    const end = Number(node.getAttribute('data-visible-end'));
    return end - start;
  });
}

async function cursorTime(page: import('@playwright/test').Page) {
  return page.getByTestId('editor').evaluate((node) => Number(node.getAttribute('data-cursor-time')));
}

async function canvasVariation(page: import('@playwright/test').Page, testId: string) {
  return page.getByTestId(testId).evaluate(async (canvas: HTMLCanvasElement) => {
    const bitmap = await createImageBitmap(canvas);
    const sampleWidth = Math.min(96, bitmap.width);
    const sampleHeight = Math.min(96, bitmap.height);
    const offscreen = new OffscreenCanvas(sampleWidth, sampleHeight);
    const ctx = offscreen.getContext('2d');
    if (!ctx) return { uniqueColors: 0 };
    ctx.drawImage(bitmap, 0, 0, sampleWidth, sampleHeight);
    const pixels = ctx.getImageData(0, 0, sampleWidth, sampleHeight).data;
    const colors = new Set<string>();
    for (let i = 0; i < pixels.length; i += 16) {
      colors.add(`${pixels[i]},${pixels[i + 1]},${pixels[i + 2]},${pixels[i + 3]}`);
    }
    return { uniqueColors: colors.size };
  });
}

async function measureScrollFrames(page: import('@playwright/test').Page) {
  return page.evaluate(async () => {
    const timeline = document.querySelector<HTMLElement>('[data-testid="timeline"]');
    if (!timeline) throw new Error('timeline missing');
    const samples: number[] = [];
    let last = performance.now();
    for (let i = 0; i < 180; i += 1) {
      timeline.dispatchEvent(
        new WheelEvent('wheel', {
          bubbles: true,
          cancelable: true,
          shiftKey: true,
          deltaY: 48
        })
      );
      await new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));
      const now = performance.now();
      samples.push(now - last);
      last = now;
    }
    samples.sort((a, b) => a - b);
    const at = (q: number) => samples[Math.min(samples.length - 1, Math.floor(q * (samples.length - 1)))] ?? 0;
    return {
      p50: at(0.5),
      p95: at(0.95),
      max: samples[samples.length - 1] ?? 0,
      samples: samples.length
    };
  });
}
