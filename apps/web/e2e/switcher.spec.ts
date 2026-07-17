import { expect, test, type Locator, type Page } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const screenshots = path.join(here, 'screenshots');
const audioDir = path.join(root, 'tests/fixtures/audio');

function payload(source: string, name: string) {
  return { name, mimeType: 'audio/wav', buffer: fs.readFileSync(path.join(audioDir, source)) };
}

// Two takes of different length so a switch is observable in the shared viewport.
const corpus = [
  payload('arctic_bdl_a0001.wav', 'arctic_bdl_a0001.wav'),
  payload('synth_vowel_a.wav', 'synth_vowel_a.wav')
];

function visibleEnd(page: Page) {
  return page
    .getByTestId('editor')
    .evaluate((node) => Number(node.getAttribute('data-visible-end')));
}

// Opens the popover from the keyboard and waits for every thumbnail to paint.
async function openAndAwaitThumbs(page: Page, popover: Locator) {
  await page.getByTestId('recording-switcher').focus();
  await page.keyboard.press('Enter');
  await expect(popover).toBeVisible();
  await expect
    .poll(() => popover.locator('[data-testid="wave-thumb"][data-painted="true"]').count(), {
      timeout: 30_000
    })
    .toBe(2);
}

test('switch recordings from the breadcrumb popover with the keyboard only', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/');

  await page.getByTestId('folder-input').setInputFiles(corpus);
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(2, { timeout: 30_000 });

  // Open the first take in the editor.
  await page.locator('[data-recording-name="arctic_bdl_a0001"]').click();
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);
  await expect(page.getByTestId('recording-switcher-name')).toHaveText('arctic_bdl_a0001');
  const firstEnd = await visibleEnd(page);

  const popover = page.getByTestId('recording-switcher-popover');

  // Screenshots of the open popover, both themes. A theme toggle is a click
  // outside the popover and so closes it; reopen for each capture.
  await openAndAwaitThumbs(page, popover);
  await page.screenshot({ path: path.join(screenshots, 'switcher-light.png'), fullPage: true });
  await page.keyboard.press('Escape');
  await expect(popover).toBeHidden();

  await page.getByLabel('Toggle theme').first().click();
  await expect(page.locator('html')).toHaveClass(/dark/);
  await openAndAwaitThumbs(page, popover);
  await page.screenshot({ path: path.join(screenshots, 'switcher-dark.png'), fullPage: true });
  await page.keyboard.press('Escape');
  await expect(popover).toBeHidden();

  await page.getByLabel('Toggle theme').first().click();
  await expect(page.locator('html')).not.toHaveClass(/dark/);

  // Keyboard-only switch: open, arrow to move the active row, filter-as-you-type
  // to the other take, Enter to commit.
  await openAndAwaitThumbs(page, popover);
  await page.keyboard.press('ArrowDown');
  await expect(popover.locator('[data-testid="switcher-option"][data-active="true"]')).toHaveCount(1);
  await page.keyboard.type('synth');
  await expect(popover.getByTestId('switcher-option')).toHaveCount(1);
  await page.keyboard.press('Enter');

  // The popover closes and every pane repoints at the second take.
  await expect(popover).toBeHidden();
  await expect(page.getByTestId('recording-switcher-name')).toHaveText('synth_vowel_a');
  await expect.poll(() => visibleEnd(page)).not.toBe(firstEnd);

  // Escape closes the popover without switching.
  await page.getByTestId('recording-switcher').focus();
  await page.keyboard.press('Enter');
  await expect(popover).toBeVisible();
  await page.keyboard.press('Escape');
  await expect(popover).toBeHidden();
  await expect(page.getByTestId('recording-switcher-name')).toHaveText('synth_vowel_a');
});
