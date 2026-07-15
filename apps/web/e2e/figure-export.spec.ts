import { expect, test, type Page } from '@playwright/test';
import { execFileSync } from 'node:child_process';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const wavFixture = path.join(root, 'tests/fixtures/audio/arctic_bdl_a0001.wav');
const screenshots = path.join(here, 'screenshots');
const figcheck = path.join(root, 'tools/figcheck/check.sh');

async function loadFixture(page: Page) {
  await page.goto('/');
  await page.getByTestId('file-input').setInputFiles(wavFixture);
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);
  await expect(page.getByTestId('tier-pane')).toHaveAttribute('data-undo-depth', '1', {
    timeout: 60_000
  });
}

async function openDialog(page: Page) {
  await page.getByTestId('open-export').click();
  await expect(page.getByTestId('export-dialog')).toBeVisible();
  await expect(page.locator('[data-testid="figure-preview"] svg')).toBeVisible({ timeout: 30_000 });
}

function previewSource(page: Page): Promise<string> {
  return page.getByTestId('figure-preview-source').evaluate((el) => el.textContent ?? '');
}

async function downloadBytes(page: Page): Promise<{ name: string; buffer: Buffer }> {
  const [download] = await Promise.all([
    page.waitForEvent('download'),
    page.getByTestId('figure-download').click()
  ]);
  const file = await download.path();
  return { name: download.suggestedFilename(), buffer: fs.readFileSync(file) };
}

test('figure export: preview equals SVG export byte for byte', async ({ page }) => {
  await loadFixture(page);
  await openDialog(page);

  const preview = await previewSource(page);
  expect(preview.length).toBeGreaterThan(0);
  expect(preview.startsWith('<svg')).toBe(true);

  // The SVG download is the same to_svg call that produced the preview, so the
  // bytes must match exactly — preview equals export by construction.
  const { name, buffer } = await downloadBytes(page);
  expect(name).toBe('figure.svg');
  expect(buffer.toString('utf8')).toBe(preview);
});

test('figure export: preview re-renders on theme and format changes', async ({ page }) => {
  await loadFixture(page);
  await openDialog(page);

  const light = await previewSource(page);
  // Flip the preview theme to dark: the spectrogram recolorizes and the
  // background changes, so the SVG source must differ.
  await page.getByTestId('figure-theme-dark').click();
  await expect
    .poll(async () => (await previewSource(page)) !== light, { timeout: 15_000 })
    .toBe(true);
  const dark = await previewSource(page);
  expect(dark).not.toBe(light);

  // Grayscale-for-print palette also changes the render.
  await page.getByTestId('figure-palette').selectOption('grayscale');
  await expect
    .poll(async () => (await previewSource(page)) !== dark, { timeout: 15_000 })
    .toBe(true);
});

test('figure export: Typst source downloads and compiles', async ({ page }) => {
  const detect = (() => {
    try {
      execFileSync('bash', [figcheck, 'typst', 'detect'], { stdio: 'ignore' });
      return true;
    } catch {
      return false;
    }
  })();

  await loadFixture(page);
  await openDialog(page);

  // A Typst figure without the spectrogram layer has no image sidecar, so the
  // download is a single self-contained .typ the compiler reads on its own.
  // Drop the spectrogram and wait for the rebuild before exporting so the
  // figure the download reads no longer carries the spectrogram sidecar.
  const base = await previewSource(page);
  await page.getByTestId('figure-layer-spectrogram').uncheck();
  await expect
    .poll(async () => (await previewSource(page)) !== base, { timeout: 15_000 })
    .toBe(true);
  await page.getByTestId('figure-format').selectOption('typst');

  const { name, buffer } = await downloadBytes(page);
  expect(name.endsWith('.typ')).toBe(true);
  expect(buffer.length).toBeGreaterThan(0);

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), 'phx-figure-'));
  const typPath = path.join(dir, name);
  fs.writeFileSync(typPath, buffer);

  test.skip(!detect, 'typst toolchain not available for compile check');
  execFileSync('bash', [figcheck, 'typst', 'run', typPath], { stdio: 'inherit' });
});

test('figure export dialog: light and dark screenshots', async ({ page }) => {
  await loadFixture(page);

  await page.getByTestId('open-export').click();
  await expect(page.getByTestId('export-dialog')).toBeVisible();

  const themeToggle = page.getByRole('button', { name: 'Toggle theme' });

  // Light: app chrome light, preview theme light.
  await page.getByTestId('figure-theme-light').click();
  await expect(page.locator('[data-testid="figure-preview"] svg')).toBeVisible({ timeout: 30_000 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(screenshots, 'export-dialog-light.png'), fullPage: true });

  // Dark: flip the app to dark chrome (if not already) and the preview to dark.
  const isDark = await page.evaluate(() => document.documentElement.classList.contains('dark'));
  if (!isDark) await themeToggle.click();
  await expect.poll(() =>
    page.evaluate(() => document.documentElement.classList.contains('dark'))
  ).toBe(true);
  await page.getByTestId('figure-theme-dark').click();
  await expect(page.locator('[data-testid="figure-preview"] svg')).toBeVisible({ timeout: 30_000 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(screenshots, 'export-dialog-dark.png'), fullPage: true });
});
