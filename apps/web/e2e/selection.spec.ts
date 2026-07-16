import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openEditorWithFixture } from './helpers';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const vowelFixture = path.join(root, 'tests/fixtures/audio/synth_vowel_a.wav');
const screenshots = path.join(here, 'screenshots');

interface BoxCoords {
  t0: number;
  t1: number;
  f0: number;
  f1: number;
}

/** Drags a time–frequency box across the spectrogram and returns its signal coordinates. */
async function dragSpectrogramBox(page: Page): Promise<BoxCoords> {
  const canvas = page.getByTestId('spectrogram-canvas');
  const box = await canvas.boundingBox();
  if (!box) throw new Error('spectrogram canvas has no box');
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
  // The readout is an async engine query; wait for it to settle before callers
  // read any measured value.
  await expect
    .poll(async () => {
      const value = await page.getByTestId('readout-band-energy').getAttribute('data-value');
      return value !== null && value !== '' && Number.isFinite(Number(value));
    }, { timeout: 30_000 })
    .toBe(true);
  const read = async (attr: string) => Number(await bar.getAttribute(attr));
  return { t0: await read('data-t0'), t1: await read('data-t1'), f0: await read('data-f0'), f1: await read('data-f1') };
}

/** The spectrogram-pane selection box (both panes render a box for a shared selection). */
function spectrogramBox(page: Page) {
  return page.getByTestId('selection-layer-box').getByTestId('selection-box');
}

test('spectrogram box selection: readout values are present and finite', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  const coords = await dragSpectrogramBox(page);

  expect(coords.t1).toBeGreaterThan(coords.t0);
  expect(coords.f1).toBeGreaterThan(coords.f0);

  const band = page.getByTestId('readout-band-energy');
  await expect(band).toBeVisible();
  const bandValue = Number(await band.getAttribute('data-value'));
  expect(Number.isFinite(bandValue)).toBe(true);

  const f0Mean = Number(await page.getByTestId('readout-f0-mean').getAttribute('data-value'));
  expect(Number.isFinite(f0Mean)).toBe(true);
  expect(f0Mean).toBeGreaterThan(0);

  await expect(page.getByTestId('readout-duration')).toContainText('s');
});

test('batch equals GUI: readout band energy equals a direct engine query', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  const coords = await dragSpectrogramBox(page);

  const shown = Number(await page.getByTestId('readout-band-energy').getAttribute('data-value'));
  expect(Number.isFinite(shown)).toBe(true);

  // Direct engine query at the identical coordinates, through the live client
  // the app drives — never a frontend recomputation.
  const direct = await page.evaluate(async (box) => {
    const hook = (globalThis as unknown as { __phonix?: { client: any; audioId: bigint | null } })
      .__phonix;
    if (!hook || hook.audioId === null) throw new Error('no client hook');
    return (await hook.client.bandEnergy(hook.audioId, box.t0, box.t1, box.f0, box.f1)) as number;
  }, coords);

  expect(Math.abs(shown - direct)).toBeLessThan(1e-6);
});

test('selection box stays anchored in signal coordinates across zoom', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  await dragSpectrogramBox(page);

  const boxEl = spectrogramBox(page);
  const beforeT0 = await boxEl.getAttribute('data-sel-t0');
  const beforeRect = await boxEl.boundingBox();
  if (!beforeRect) throw new Error('selection box not rendered');
  const beforeSpanEnd = await page.getByTestId('editor').getAttribute('data-visible-end');

  // Zoom in ~4x centred on the box, then confirm the box kept its signal-space
  // start while its pixel position re-mapped under the new viewport.
  const canvas = page.getByTestId('spectrogram-canvas');
  const cbox = await canvas.boundingBox();
  if (!cbox) throw new Error('no canvas');
  await page.mouse.move(cbox.x + cbox.width * 0.5, cbox.y + cbox.height * 0.5);
  for (let i = 0; i < 7; i += 1) await page.mouse.wheel(0, -100);

  await expect
    .poll(async () => page.getByTestId('editor').getAttribute('data-visible-end'))
    .not.toBe(beforeSpanEnd);

  await expect(boxEl).toHaveAttribute('data-sel-t0', beforeT0 ?? '');
  const afterRect = await boxEl.boundingBox();
  expect(afterRect).not.toBeNull();
  expect(Math.abs((afterRect?.x ?? 0) - beforeRect.x)).toBeGreaterThan(1);
});

test('voice report on the sustained vowel reports plausible values', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  await dragSpectrogramBox(page);

  await page.getByTestId('selection-voice-report').click();
  const card = page.getByTestId('voice-report-card');
  await expect(card).toBeVisible();
  const values = page.getByTestId('voice-report-values');
  await expect(values).toBeVisible({ timeout: 30_000 });

  const jitter = Number(await values.getAttribute('data-jitter-local'));
  const shimmer = Number(await values.getAttribute('data-shimmer-local'));
  const hnr = Number(await values.getAttribute('data-hnr'));

  // A clean synthetic vowel: perturbation near its ~0 injection level, HNR high.
  expect(jitter).toBeGreaterThanOrEqual(0);
  expect(jitter).toBeLessThan(0.05);
  expect(shimmer).toBeGreaterThanOrEqual(0);
  expect(shimmer).toBeLessThan(0.2);
  expect(hnr).toBeGreaterThan(10);
});

test('selection and voice report: light and dark screenshots', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  await dragSpectrogramBox(page);
  await page.getByTestId('selection-voice-report').click();
  await expect(page.getByTestId('voice-report-values')).toBeVisible({ timeout: 30_000 });

  // Light.
  await page.waitForTimeout(300);
  await page.screenshot({ path: path.join(screenshots, 'selection-voice-report-light.png'), fullPage: true });

  // Dark: close the card (Escape keeps the selection), flip to dark, re-open.
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('voice-report-card')).toHaveCount(0);
  await page.getByRole('button', { name: 'Toggle theme' }).click();
  await expect
    .poll(() => page.evaluate(() => document.documentElement.classList.contains('dark')))
    .toBe(true);
  await page.getByTestId('selection-voice-report').click();
  await expect(page.getByTestId('voice-report-values')).toBeVisible({ timeout: 30_000 });
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(screenshots, 'selection-voice-report-dark.png'), fullPage: true });
});
