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
