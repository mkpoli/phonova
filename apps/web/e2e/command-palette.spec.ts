import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openEditorWithFixture } from './helpers';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const wavFixture = path.join(root, 'tests/fixtures/audio/arctic_bdl_a0001.wav');
const screenshots = path.join(here, 'screenshots');

function palette(page: Page) {
  return page.getByTestId('command-palette');
}

function item(page: Page, id: string) {
  return page.locator(`[data-testid="command-item"][data-command-id="${id}"]`);
}

async function openPalette(page: Page) {
  await page.keyboard.press('Control+k');
  await expect(palette(page)).toBeVisible();
}

/** Types a query, waits for the target to rank in, then runs it with Enter. */
async function runCommand(page: Page, id: string, query: string) {
  await openPalette(page);
  await page.getByTestId('command-palette-input').fill(query);
  await expect(item(page, id)).toBeVisible();
  // Move the highlight onto the target so Enter runs exactly it.
  await item(page, id).hover();
  await expect(item(page, id)).toHaveAttribute('data-selected', 'true');
  await page.keyboard.press('Enter');
  await expect(palette(page)).toHaveCount(0);
}

async function loadEditor(page: Page) {
  await openEditorWithFixture(page, wavFixture);
  // Opening the recording attaches an empty annotation document; the attach
  // command queues behind the whole-signal analyses in the engine worker.
  await expect(page.getByTestId('tier-pane')).toHaveAttribute('data-undo-depth', '1', {
    timeout: 60_000
  });
}

test('context filtering: annotation actions are absent on the home screen', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByTestId('home')).toBeVisible();
  await openPalette(page);

  // Home registers project-level and appearance actions.
  await expect(item(page, 'importAudioFiles')).toBeVisible();
  await expect(item(page, 'switchTheme')).toBeVisible();

  // The editor and annotation surfaces are unmounted, so their actions are not
  // registered and cannot appear.
  await expect(item(page, 'addIntervalTier')).toHaveCount(0);
  await expect(item(page, 'playPause')).toHaveCount(0);
  await expect(item(page, 'exportFigure')).toHaveCount(0);

  await page.keyboard.press('Escape');
  await expect(palette(page)).toHaveCount(0);
});

test('palette runs the same code paths as buttons and keys', async ({ page }) => {
  await loadEditor(page);
  const editor = page.getByTestId('editor');

  // Play / pause — the cursor is engine-clock driven, so it advances while playing.
  const before = Number(await editor.getAttribute('data-cursor-time'));
  await runCommand(page, 'playPause', 'play');
  await page.waitForTimeout(600);
  await runCommand(page, 'playPause', 'play');
  const after = Number(await editor.getAttribute('data-cursor-time'));
  expect(after).toBeGreaterThan(before);

  // Toggle a track overlay — the inspector checkbox reflects the shared state.
  await expect(page.getByTestId('toggle-pitch')).toBeChecked();
  await runCommand(page, 'togglePitchTrack', 'toggle pitch');
  await expect(page.getByTestId('toggle-pitch')).not.toBeChecked();

  // Add an interval tier through the palette.
  await expect(page.getByTestId('tier-pane')).toHaveAttribute('data-tier-count', '0');
  await runCommand(page, 'addIntervalTier', 'interval');
  await expect(page.getByTestId('tier-pane')).toHaveAttribute('data-tier-count', '1');
  await expect(page.getByTestId('tier-pane')).toHaveAttribute('data-undo-depth', '2');

  // Undo the tier from the palette.
  await runCommand(page, 'undo', 'undo');
  await expect(page.getByTestId('tier-pane')).toHaveAttribute('data-tier-count', '0');
  await expect(page.getByTestId('tier-pane')).toHaveAttribute('data-undo-depth', '1');

  // Open the figure export dialog.
  await expect(page.getByTestId('export-dialog')).toHaveCount(0);
  await runCommand(page, 'exportFigure', 'export figure');
  await expect(page.getByTestId('export-dialog')).toBeVisible();
  await page.getByTestId('export-close').click();

  // Switch theme — the class on the root element flips.
  await expect
    .poll(() => page.evaluate(() => document.documentElement.classList.contains('dark')))
    .toBe(false);
  await runCommand(page, 'switchTheme', 'theme');
  await expect
    .poll(() => page.evaluate(() => document.documentElement.classList.contains('dark')))
    .toBe(true);
  // Back to light for the deterministic screenshot pass below.
  await runCommand(page, 'switchTheme', 'theme');
});

test('zoom to selection is context-gated and reframes the viewport', async ({ page }) => {
  await loadEditor(page);
  const editor = page.getByTestId('editor');

  // With no selection the action is not offered.
  await openPalette(page);
  await page.getByTestId('command-palette-input').fill('zoom to selection');
  await expect(item(page, 'zoomToSelection')).toHaveCount(0);
  await page.keyboard.press('Escape');

  // Draw a time–frequency box on the spectrogram.
  const canvas = page.getByTestId('spectrogram-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('spectrogram canvas has no box');
  const x0 = box.x + box.width * 0.35;
  const x1 = box.x + box.width * 0.65;
  const y0 = box.y + box.height * 0.3;
  const y1 = box.y + box.height * 0.7;
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  await page.mouse.move((x0 + x1) / 2, (y0 + y1) / 2, { steps: 4 });
  await page.mouse.move(x1, y1, { steps: 4 });
  await page.mouse.up();
  await expect(page.getByTestId('readout-bar')).toBeVisible();

  const startBefore = Number(await editor.getAttribute('data-visible-start'));
  await runCommand(page, 'zoomToSelection', 'zoom to selection');
  await expect
    .poll(() => editor.getAttribute('data-visible-start').then(Number))
    .toBeGreaterThan(startBefore);
});

test('open palette: light and dark screenshots', async ({ page }) => {
  await loadEditor(page);

  await openPalette(page);
  await page.getByTestId('command-palette-input').fill('');
  await expect(item(page, 'addIntervalTier')).toBeVisible();
  await page.screenshot({ path: path.join(screenshots, 'command-palette-light.png'), fullPage: true });

  await page.keyboard.press('Escape');
  await runCommand(page, 'switchTheme', 'theme');
  await openPalette(page);
  await expect(item(page, 'addIntervalTier')).toBeVisible();
  await page.screenshot({ path: path.join(screenshots, 'command-palette-dark.png'), fullPage: true });
});
