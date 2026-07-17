import { expect, test, type Page } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';

// A recording well past the engine's two-minute eager threshold, so the import
// takes the streamed path: persisted to OPFS, opened over a synchronous access
// handle, waveform and spectrogram served from ranged reads. The whole decoded
// signal never resides in memory, so the corpus appears at header speed and
// scrolling stays smooth over a file that would be hundreds of megabytes
// decoded.
const SAMPLE_RATE = 48_000;
const MINUTES = 10;

/** Builds a 16-bit mono PCM WAV of `seconds` at `SAMPLE_RATE`. */
function makeWav(seconds: number): Buffer {
  const frames = SAMPLE_RATE * seconds;
  const dataLen = frames * 2;
  const buf = Buffer.alloc(44 + dataLen);
  buf.write('RIFF', 0, 'ascii');
  buf.writeUInt32LE(36 + dataLen, 4);
  buf.write('WAVE', 8, 'ascii');
  buf.write('fmt ', 12, 'ascii');
  buf.writeUInt32LE(16, 16);
  buf.writeUInt16LE(1, 20); // PCM
  buf.writeUInt16LE(1, 22); // mono
  buf.writeUInt32LE(SAMPLE_RATE, 24);
  buf.writeUInt32LE(SAMPLE_RATE * 2, 28);
  buf.writeUInt16LE(2, 32);
  buf.writeUInt16LE(16, 34);
  buf.write('data', 36, 'ascii');
  buf.writeUInt32LE(dataLen, 40);
  // A slow chirp so the waveform carries real min/max structure at every zoom.
  for (let i = 0; i < frames; i += 1) {
    const t = i / SAMPLE_RATE;
    const s = Math.sin(2 * Math.PI * (110 + 5 * t) * t) * 0.8;
    buf.writeInt16LE(Math.round(s * 32_000), 44 + i * 2);
  }
  return buf;
}

async function drawGeneration(page: Page, testId: string): Promise<number> {
  return page
    .getByTestId(testId)
    .evaluate((node) => Number(node.getAttribute('data-draw-generation')));
}

async function dispatchWheel(page: Page, deltaY: number, shiftKey = false): Promise<void> {
  await page.getByTestId('timeline').evaluate(
    (node, { deltaY, shiftKey }) => {
      const rect = node.getBoundingClientRect();
      node.dispatchEvent(
        new WheelEvent('wheel', {
          bubbles: true,
          cancelable: true,
          deltaY,
          shiftKey,
          clientX: rect.left + rect.width / 2,
          clientY: rect.top + rect.height / 2
        })
      );
    },
    { deltaY, shiftKey }
  );
}

let fixturePath = '';

test.beforeAll(() => {
  const wav = makeWav(MINUTES * 60);
  fixturePath = path.join(fs.mkdtempSync(path.join(os.tmpdir(), 'phx-streamed-')), 'long_take.wav');
  fs.writeFileSync(fixturePath, wav);
  console.log(`streamed-import fixture: ${(wav.byteLength / (1024 * 1024)).toFixed(1)} MiB WAV`);
});

test.afterAll(() => {
  if (fixturePath) fs.rmSync(path.dirname(fixturePath), { recursive: true, force: true });
});

test('a 10-minute recording imports streamed: corpus at header speed, smooth scroll', async ({
  page
}) => {
  await page.goto('/');
  await expect(page.getByTestId('home-empty')).toBeVisible();

  // Time-to-corpus: from selecting the file to the recording row appearing. The
  // streamed open reads only the header and builds the bounded pyramid, so this
  // must not scale with the decoded footprint.
  const started = Date.now();
  await page.getByTestId('folder-input').setInputFiles([fixturePath]);
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(1, { timeout: 30_000 });
  const timeToCorpus = Date.now() - started;
  console.log(`streamed-import time-to-corpus: ${timeToCorpus} ms`);

  // Metadata reads out at header speed: the duration cell reflects the true
  // length without a full decode.
  const durationText = await page.getByTestId('corpus-row').first().textContent();
  console.log(`streamed-import row: ${durationText?.replace(/\s+/g, ' ').trim()}`);

  expect(timeToCorpus).toBeLessThan(3000);

  // Main-thread heap proxy for the RSS bound (the authoritative figure comes
  // from the native long_file_stream test; the whole file is never held here).
  const heapMiB = await page.evaluate(() => {
    const mem = (performance as unknown as { memory?: { usedJSHeapSize: number } }).memory;
    return mem ? mem.usedJSHeapSize / (1024 * 1024) : null;
  });
  console.log(
    `streamed-import main-thread heap: ${heapMiB === null ? 'unavailable' : `${heapMiB.toFixed(1)} MiB`}`
  );

  // Open the editor and confirm the panes paint over the streamed source.
  await page.getByTestId('corpus-row').first().click();
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);
  await expect(page.getByTestId('waveform-canvas')).toHaveAttribute('data-render-token', /[1-9]/);
  await expect(page.getByTestId('spectrogram-canvas')).toHaveAttribute(
    'data-render-token',
    /[1-9]/
  );

  // Zoom in so a pan actually moves, then assert the panes remap within a frame
  // — the transform-first redraw never waits on a worker round-trip, so a
  // streamed source scrolls as smoothly as an eager one.
  await page.getByTestId('timeline').hover();
  await dispatchWheel(page, -500);
  await dispatchWheel(page, -500);
  await page.waitForTimeout(200);

  const waveGen0 = await drawGeneration(page, 'waveform-canvas');
  const specGen0 = await drawGeneration(page, 'spectrogram-canvas');
  const panStart = Date.now();
  await dispatchWheel(page, 200, true);
  await expect
    .poll(() => drawGeneration(page, 'spectrogram-canvas'), { timeout: 100 })
    .toBeGreaterThan(specGen0);
  const panLatency = Date.now() - panStart;
  console.log(`streamed-import pan latency: ${panLatency} ms`);
  expect(panLatency).toBeLessThan(100);
  await expect.poll(() => drawGeneration(page, 'waveform-canvas')).toBeGreaterThan(waveGen0);
});
