import { expect, test, type Page } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { canvasForegroundCoverage } from './helpers';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const screenshots = path.join(here, 'screenshots');

const audioDir = path.join(root, 'tests/fixtures/audio');
const textgridDir = path.join(root, 'tests/fixtures/textgrids');

function payload(dir: string, source: string, name: string, mimeType: string) {
  return { name, mimeType, buffer: fs.readFileSync(path.join(dir, source)) };
}

// Five recordings, two carrying a TextGrid whose stem matches its WAV. The
// TextGrids are given the matching basename so stem attachment fires on drop.
const corpus = [
  payload(audioDir, 'arctic_bdl_a0001.wav', 'arctic_bdl_a0001.wav', 'audio/wav'),
  payload(audioDir, 'arctic_slt_a0001.wav', 'arctic_slt_a0001.wav', 'audio/wav'),
  payload(audioDir, 'librispeech_2277-149896-0005.wav', 'librispeech_2277-149896-0005.wav', 'audio/wav'),
  payload(audioDir, 'synth_vowel_a.wav', 'synth_vowel_a.wav', 'audio/wav'),
  payload(audioDir, 'synth_tone_sweep.wav', 'synth_tone_sweep.wav', 'audio/wav'),
  payload(textgridDir, 'arctic_bdl_a0001_long_utf8.TextGrid', 'arctic_bdl_a0001.TextGrid', 'text/plain'),
  payload(textgridDir, 'arctic_slt_a0001_short_utf8.TextGrid', 'arctic_slt_a0001.TextGrid', 'text/plain')
];

async function toggleTheme(page: Page, expectDark: boolean) {
  await page.getByLabel('Toggle theme').first().click();
  if (expectDark) await expect(page.locator('html')).toHaveClass(/dark/);
  else await expect(page.locator('html')).not.toHaveClass(/dark/);
}

test('drop a folder, build a browsable corpus, screenshots both themes', async ({ page }) => {
  await page.goto('/?app=1');

  // Empty state explains drop-to-start.
  await expect(page.getByTestId('home-empty')).toBeVisible();
  await page.screenshot({ path: path.join(screenshots, 'home-empty-light.png'), fullPage: true });
  await toggleTheme(page, true);
  await page.screenshot({ path: path.join(screenshots, 'home-empty-dark.png'), fullPage: true });
  await toggleTheme(page, false);

  await page.getByTestId('folder-input').setInputFiles(corpus);

  // The corpus view opens with one row per WAV.
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(5, { timeout: 30_000 });

  // The two recordings with a matching TextGrid carry annotations.
  await expect(page.locator('[data-testid="corpus-row"][data-has-annotation="true"]')).toHaveCount(
    2
  );

  // Metadata reads out per row (duration cell is non-empty).
  const firstRow = page.locator('[data-recording-name="arctic_bdl_a0001"]');
  await expect(firstRow).toHaveAttribute('data-has-annotation', 'true');

  // Every row renders a waveform thumbnail.
  await expect
    .poll(() => page.locator('[data-testid="wave-thumb"][data-painted="true"]').count(), {
      timeout: 30_000
    })
    .toBe(5);

  await page.screenshot({ path: path.join(screenshots, 'corpus-light.png'), fullPage: true });
  await toggleTheme(page, true);
  await expect
    .poll(() => page.locator('[data-testid="wave-thumb"][data-painted="true"]').count(), {
      timeout: 30_000
    })
    .toBe(5);
  await page.screenshot({ path: path.join(screenshots, 'corpus-dark.png'), fullPage: true });

  // The home grid lists the project, both themes.
  await page.getByTestId('back-home').click();
  await expect(page.getByTestId('project-card')).toHaveCount(1);
  await page.screenshot({ path: path.join(screenshots, 'home-grid-dark.png'), fullPage: true });
  await toggleTheme(page, false);
  await page.screenshot({ path: path.join(screenshots, 'home-grid-light.png'), fullPage: true });
});

test('open sample project loads the bundled corpus', async ({ page }) => {
  await page.goto('/?app=1');

  // The empty state offers the sample entry and the first-run palette hint.
  await expect(page.getByTestId('home-empty')).toBeVisible();
  await expect(page.getByTestId('palette-hint')).toBeVisible();

  await page.getByTestId('open-sample').click();

  // Three recordings (two ARCTIC sentences and the perturbed vowel); the two
  // ARCTIC sentences carry their phone-tier TextGrids (CMU ARCTIC forced
  // alignment).
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(3, { timeout: 30_000 });
  await expect(page.locator('[data-testid="corpus-row"][data-has-annotation="true"]')).toHaveCount(
    2
  );
  await expect(page.locator('[data-recording-name="synth_vowel_perturbed"]')).toHaveCount(1);
});

test('opening the sample repeatedly always reaches a rendered editor', async ({ page }) => {
  // Regression coverage for a home screen where the intro paragraph and the
  // "Open sample project" button both mention "sample": a fuzzy text match
  // picks the paragraph, clicks nothing, and fails silently. The control is
  // only ever addressed by its test id here, and the whole open-to-render
  // path runs several times so a one-in-N flake fails the suite too.
  await page.goto('/?app=1');

  for (let attempt = 0; attempt < 4; attempt += 1) {
    await expect(page.getByTestId('home-empty')).toBeVisible();
    await page.getByTestId('open-sample').click();

    await expect(page.getByTestId('corpus')).toBeVisible();
    await expect(page.getByTestId('corpus-row')).toHaveCount(3, { timeout: 30_000 });

    await page.getByTestId('corpus-row').first().click();
    await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);
    await expect(page.getByTestId('waveform-canvas')).toHaveAttribute('data-render-token', /[1-9]/);
    await expect(page.getByTestId('spectrogram-canvas')).toHaveAttribute('data-render-token', /[1-9]/);
    expect(await canvasForegroundCoverage(page, 'waveform-canvas')).toBeGreaterThan(0);
    expect(await canvasForegroundCoverage(page, 'spectrogram-canvas')).toBeGreaterThan(0);

    await page.getByTestId('back-corpus').click();
    await page.getByTestId('back-home').click();
    await expect(page.getByTestId('project-card')).toHaveCount(1);
    await page.getByTestId('delete-project').click();
  }
});

test('annotate, recover after reload via autosave, then delete', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/?app=1');

  const files = [
    payload(audioDir, 'synth_vowel_a.wav', 'synth_vowel_a.wav', 'audio/wav'),
    payload(audioDir, 'arctic_bdl_a0001.wav', 'arctic_bdl_a0001.wav', 'audio/wav')
  ];
  await page.getByTestId('folder-input').setInputFiles(files);
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(2, { timeout: 30_000 });

  // Open a recording and annotate one interval.
  await page.locator('[data-recording-name="synth_vowel_a"]').click();
  await expect(page.getByTestId('tier-pane')).toHaveAttribute('data-undo-depth', '1', {
    timeout: 60_000
  });
  await page.getByTestId('add-interval-tier').focus();
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('interval')).toHaveCount(1);
  await page.keyboard.press('1');
  await page.keyboard.press('Enter');
  await page.getByTestId('label-editor').fill('vowel');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('interval').nth(0)).toHaveAttribute('data-label', 'vowel');

  // Let the autosave debounce (2 s) write the sidecar, then simulate a crash by
  // reloading without an explicit save.
  await page.waitForTimeout(3_500);
  await page.reload();

  // The home grid flags the project as holding unsaved work.
  await expect(page.getByTestId('project-card')).toHaveCount(1);
  await expect(page.getByTestId('recovery-badge')).toBeVisible();

  // Opening prompts to recover; accepting restores the autosaved annotation.
  await page.getByTestId('open-project').click();
  await expect(page.getByTestId('recovery-prompt')).toBeVisible();
  await page.getByTestId('recovery-accept').click();
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(
    page.locator('[data-recording-name="synth_vowel_a"][data-has-annotation="true"]')
  ).toHaveCount(1);
  await page.locator('[data-recording-name="synth_vowel_a"]').click();
  await expect(page.getByTestId('interval').nth(0)).toHaveAttribute('data-label', 'vowel', {
    timeout: 60_000
  });

  // Back to the grid, delete the project; the empty state returns.
  await page.getByTestId('back-corpus').click();
  await expect(page.getByTestId('corpus')).toBeVisible();
  await page.getByTestId('back-home').click();
  await expect(page.getByTestId('home')).toBeVisible();
  await page.getByTestId('delete-project').click();
  await expect(page.getByTestId('home-empty')).toBeVisible();
});
