import { expect, test, type Browser, type Download, type Page } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openEditorWithFixture } from './helpers';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const screenshots = path.join(here, 'screenshots');
const audioDir = path.join(root, 'tests/fixtures/audio');
const textgridDir = path.join(root, 'tests/fixtures/textgrids');
const vowelFixture = path.join(audioDir, 'synth_vowel_a.wav');

function payload(dir: string, source: string, name: string, mimeType: string) {
  return { name, mimeType, buffer: fs.readFileSync(path.join(dir, source)) };
}

async function captureDownload(page: Page, trigger: () => Promise<void>): Promise<Uint8Array> {
  const [download] = await Promise.all([page.waitForEvent('download'), trigger()]);
  return readDownload(download);
}

async function readDownload(download: Download): Promise<Uint8Array> {
  const file = await download.path();
  if (!file) throw new Error('download produced no file');
  return new Uint8Array(fs.readFileSync(file));
}

/** Reads a WAV's channel count, sample rate, and sample-accurate duration. */
function parseWav(bytes: Uint8Array): { sampleRate: number; channels: number; duration: number } {
  const view = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  const tag = (offset: number) => String.fromCharCode(...bytes.subarray(offset, offset + 4));
  if (tag(0) !== 'RIFF' || tag(8) !== 'WAVE') throw new Error('not a RIFF/WAVE file');
  let sampleRate = 0;
  let channels = 0;
  let bitsPerSample = 0;
  let dataSize = 0;
  let offset = 12;
  while (offset + 8 <= bytes.length) {
    const id = tag(offset);
    const size = view.getUint32(offset + 4, true);
    const body = offset + 8;
    if (id === 'fmt ') {
      channels = view.getUint16(body + 2, true);
      sampleRate = view.getUint32(body + 4, true);
      bitsPerSample = view.getUint16(body + 14, true);
    } else if (id === 'data') {
      dataSize = size;
    }
    offset = body + size + (size % 2);
  }
  const bytesPerFrame = channels * (bitsPerSample / 8);
  const frames = bytesPerFrame > 0 ? dataSize / bytesPerFrame : 0;
  return { sampleRate, channels, duration: frames / sampleRate };
}

async function toggleTheme(page: Page, dark: boolean) {
  await page.getByLabel('Toggle theme').first().click();
  if (dark) await expect(page.locator('html')).toHaveClass(/dark/);
  else await expect(page.locator('html')).not.toHaveClass(/dark/);
}

test('project bundle round-trips groups, tags, and annotations across a wiped profile', async ({
  page,
  browser
}: {
  page: Page;
  browser: Browser;
}) => {
  test.setTimeout(120_000);
  await page.goto('/?app=1');

  // A project with two recordings, one annotated by its TextGrid.
  await page.getByTestId('folder-input').setInputFiles([
    payload(audioDir, 'arctic_bdl_a0001.wav', 'arctic_bdl_a0001.wav', 'audio/wav'),
    payload(textgridDir, 'arctic_bdl_a0001_long_utf8.TextGrid', 'arctic_bdl_a0001.TextGrid', 'text/plain'),
    payload(audioDir, 'synth_vowel_a.wav', 'synth_vowel_a.wav', 'audio/wav')
  ]);
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(2, { timeout: 30_000 });

  // A group in the tree, and a project-level tag.
  await page.getByTestId('new-group').click();
  await expect(page.getByTestId('rename-group-name')).toHaveText('New group');
  await page.getByTestId('project-details').click();
  await page.getByTestId('metadata-tag-input').fill('fieldwork');
  await page.getByTestId('metadata-tag-input').press('Enter');
  await expect(page.getByTestId('metadata-tags')).toContainText('fieldwork');
  await page.getByTestId('metadata-close').click();

  // The export dialog, defaulting to a self-contained bundle. Screenshot both themes.
  await page.getByTestId('export-project').click();
  await expect(page.getByTestId('project-export')).toBeVisible();
  await expect(page.getByTestId('export-mode-bundle')).toBeChecked();
  await page.screenshot({ path: path.join(screenshots, 'project-export-light.png'), fullPage: true });
  await page.getByTestId('project-export-close').click();
  await toggleTheme(page, true);
  await page.getByTestId('export-project').click();
  await page.screenshot({ path: path.join(screenshots, 'project-export-dark.png'), fullPage: true });

  // Download the self-contained bundle.
  const bundle = await captureDownload(page, () => page.getByTestId('project-export-download').click());
  expect(bundle.byteLength).toBeGreaterThan(0);

  // A fresh browser profile: empty storage, no media on disk.
  const fresh = await browser.newContext();
  try {
    const page2 = await fresh.newPage();
    await page2.goto('/');
    await expect(page2.getByTestId('home-empty')).toBeVisible();
    await expect(page2.getByTestId('open-project-file')).toBeVisible();

    await page2
      .getByTestId('project-file-input')
      .setInputFiles({ name: 'fieldwork.phxproj', mimeType: 'application/zip', buffer: Buffer.from(bundle) });

    // Full restoration: both corpus rows, the group, the annotation, and the tag.
    await expect(page2.getByTestId('corpus')).toBeVisible();
    await expect(page2.getByTestId('corpus-row')).toHaveCount(2, { timeout: 30_000 });
    await expect(page2.getByTestId('rename-group-name')).toHaveText('New group');
    await expect(
      page2.locator('[data-recording-name="arctic_bdl_a0001"][data-has-annotation="true"]')
    ).toHaveCount(1);
    await expect
      .poll(() => page2.locator('[data-testid="wave-thumb"][data-painted="true"]').count(), {
        timeout: 30_000
      })
      .toBe(2);
    await page2.screenshot({ path: path.join(screenshots, 'project-import-light.png'), fullPage: true });
    await toggleTheme(page2, true);
    await page2.screenshot({ path: path.join(screenshots, 'project-import-dark.png'), fullPage: true });
    await toggleTheme(page2, false);

    await page2.getByTestId('project-details').click();
    await expect(page2.getByTestId('metadata-tags')).toContainText('fieldwork');
    await page2.getByTestId('metadata-close').click();

    // The restored recording opens and its tiers are present.
    await page2.locator('[data-recording-name="arctic_bdl_a0001"]').click();
    await expect(page2.getByTestId('editor')).toBeVisible();
    await expect(page2.getByTestId('interval').first()).toBeVisible({ timeout: 60_000 });
  } finally {
    await fresh.close();
  }
});

test('references-only export re-links by hash against media already present', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/?app=1');
  await page.getByTestId('folder-input').setInputFiles([
    payload(audioDir, 'synth_vowel_a.wav', 'synth_vowel_a.wav', 'audio/wav')
  ]);
  await expect(page.getByTestId('corpus-row')).toHaveCount(1, { timeout: 30_000 });

  // Export references-only: the manifest, no embedded audio.
  await page.getByTestId('export-project').click();
  await page.getByTestId('export-mode-references').check();
  const refs = await captureDownload(page, () => page.getByTestId('project-export-download').click());

  // Importing re-links the recording by content hash against the copy already
  // in OPFS, so the new project's recording resolves and opens.
  await page.getByTestId('back-home').click();
  await page
    .getByTestId('project-file-input')
    .setInputFiles({ name: 'refs.phxproj', mimeType: 'application/zip', buffer: Buffer.from(refs) });
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(1, { timeout: 30_000 });
  await page.getByTestId('corpus-row').first().click();
  await expect(page.getByTestId('editor')).toBeVisible();
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/, {
    timeout: 30_000
  });
});

test('audio export writes a valid WAV whose duration matches the range', async ({ page }) => {
  test.setTimeout(120_000);
  await openEditorWithFixture(page, vowelFixture);
  const fileDuration = Number(await page.getByTestId('editor').getAttribute('data-visible-end'));
  expect(fileDuration).toBeGreaterThan(0);

  // Whole recording: the exported WAV parses and covers the full duration.
  await page.getByTestId('open-audio-export').click();
  await expect(page.getByTestId('audio-export')).toBeVisible();
  await page.screenshot({ path: path.join(screenshots, 'audio-export-light.png'), fullPage: true });
  const wholeBytes = await captureDownload(page, () =>
    page.getByTestId('audio-export-download').click()
  );
  const whole = parseWav(wholeBytes);
  expect(whole.sampleRate).toBeGreaterThan(0);
  expect(Math.abs(whole.duration - fileDuration)).toBeLessThan(0.02);

  // A spectrogram box selection, exported to WAV: duration matches the box span.
  const canvas = page.getByTestId('spectrogram-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('no spectrogram box');
  const x0 = box.x + box.width * 0.3;
  const x1 = box.x + box.width * 0.7;
  const y0 = box.y + box.height * 0.3;
  const y1 = box.y + box.height * 0.7;
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  await page.mouse.move((x0 + x1) / 2, (y0 + y1) / 2, { steps: 4 });
  await page.mouse.move(x1, y1, { steps: 4 });
  await page.mouse.up();
  const bar = page.getByTestId('readout-bar');
  await expect(bar).toBeVisible();
  const selT0 = Number(await bar.getAttribute('data-t0'));
  const selT1 = Number(await bar.getAttribute('data-t1'));
  const selDuration = selT1 - selT0;
  expect(selDuration).toBeGreaterThan(0);

  await toggleTheme(page, true);
  await page.getByTestId('open-audio-export').click();
  await page.getByTestId('audio-scope-selection').check();
  await expect(page.getByTestId('audio-filtered')).toBeVisible();
  await page.screenshot({ path: path.join(screenshots, 'audio-export-dark.png'), fullPage: true });
  const selectionBytes = await captureDownload(page, () =>
    page.getByTestId('audio-export-download').click()
  );
  const selection = parseWav(selectionBytes);
  expect(Math.abs(selection.duration - selDuration)).toBeLessThan(0.02);

  // The band-filtered selection export is a valid mono WAV of the same span.
  await page.getByTestId('open-audio-export').click();
  await page.getByTestId('audio-scope-selection').check();
  await page.getByTestId('audio-filtered').check();
  const filteredBytes = await captureDownload(page, () =>
    page.getByTestId('audio-export-download').click()
  );
  const filtered = parseWav(filteredBytes);
  expect(filtered.channels).toBe(1);
  expect(Math.abs(filtered.duration - selDuration)).toBeLessThan(0.02);
});
