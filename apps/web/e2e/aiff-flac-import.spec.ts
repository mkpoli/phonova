import { expect, test } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { canvasForegroundCoverage, openEditorWithFixture } from './helpers';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const aiffFixture = path.join(root, 'tests/fixtures/audio/aiff_stereo_16_44100.aiff');
const flacFixture = path.join(root, 'tests/fixtures/audio/flac_level5.flac');

test('drop an AIFF file: corpus tags it, waveform and spectrogram render', async ({ page }) => {
  await openEditorWithFixture(page, aiffFixture);
  await expect(page.getByTestId('waveform-canvas')).toHaveAttribute('data-render-token', /[1-9]/);
  await expect(page.getByTestId('spectrogram-canvas')).toHaveAttribute(
    'data-render-token',
    /[1-9]/
  );
  expect(await canvasForegroundCoverage(page, 'waveform-canvas')).toBeGreaterThan(0);
  expect(await canvasForegroundCoverage(page, 'spectrogram-canvas')).toBeGreaterThan(0);

  await page.getByTestId('back-corpus').click();
  await expect(page.getByTestId('corpus-format').first()).toHaveText('AIFF');
});

test('drop a FLAC file: corpus tags it, waveform and spectrogram render', async ({ page }) => {
  await openEditorWithFixture(page, flacFixture);
  await expect(page.getByTestId('waveform-canvas')).toHaveAttribute('data-render-token', /[1-9]/);
  await expect(page.getByTestId('spectrogram-canvas')).toHaveAttribute(
    'data-render-token',
    /[1-9]/
  );
  expect(await canvasForegroundCoverage(page, 'waveform-canvas')).toBeGreaterThan(0);
  expect(await canvasForegroundCoverage(page, 'spectrogram-canvas')).toBeGreaterThan(0);

  await page.getByTestId('back-corpus').click();
  await expect(page.getByTestId('corpus-format').first()).toHaveText('FLAC');
});

test('dropping an unrecognized file reports a clear error naming it', async ({ page }) => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'phx-bad-import-'));
  const badFile = path.join(dir, 'notes.txt');
  fs.writeFileSync(badFile, 'not an audio file');

  await page.goto('/');
  await page.getByTestId('folder-input').setInputFiles([badFile]);
  await expect(page.getByTestId('error')).toBeVisible();
  await expect(page.getByTestId('error')).toContainText('notes.txt');

  fs.rmSync(dir, { recursive: true, force: true });
});
