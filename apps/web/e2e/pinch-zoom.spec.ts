import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openEditorWithFixture } from './helpers';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const shortFixture = path.join(root, 'tests/fixtures/audio/arctic_bdl_a0001.wav');

/** Reads the editor's shared viewport (time window and frequency ceiling). */
async function viewport(page: Page) {
  return page.getByTestId('editor').evaluate((node) => ({
    span:
      Number(node.getAttribute('data-visible-end')) -
      Number(node.getAttribute('data-visible-start')),
    freq: Number(node.getAttribute('data-visible-freq'))
  }));
}

async function renderToken(page: Page, testId: string) {
  return page
    .getByTestId(testId)
    .evaluate((node) => Number(node.getAttribute('data-render-token')));
}

/**
 * Dispatches a wheel on the timeline with the given modifiers. A macOS trackpad
 * pinch reaches the page as exactly this: a `wheel` event with `ctrlKey` set.
 */
async function timelineWheel(
  page: Page,
  deltaY: number,
  modifiers: { ctrlKey?: boolean; altKey?: boolean }
) {
  await page.getByTestId('timeline').evaluate(
    (node, { deltaY, modifiers }) => {
      const rect = node.getBoundingClientRect();
      node.dispatchEvent(
        new WheelEvent('wheel', {
          bubbles: true,
          cancelable: true,
          deltaY,
          clientX: rect.left + rect.width / 2,
          clientY: rect.top + rect.height / 2,
          ...modifiers
        })
      );
    },
    { deltaY, modifiers }
  );
}

test('trackpad pinch drives synced time zoom, not frequency zoom', async ({ page }) => {
  await openEditorWithFixture(page, shortFixture);
  await expect(page.getByTestId('waveform-canvas')).toHaveAttribute('data-render-token', /[1-9]/);
  await expect(page.getByTestId('spectrogram-canvas')).toHaveAttribute('data-render-token', /[1-9]/);

  const before = await viewport(page);
  const waveTokenBefore = await renderToken(page, 'waveform-canvas');
  const spectroTokenBefore = await renderToken(page, 'spectrogram-canvas');

  // A pinch-in: Ctrl wheel with a negative delta. It must zoom time in, leaving
  // the frequency ceiling untouched.
  await timelineWheel(page, -500, { ctrlKey: true });
  await expect.poll(() => viewport(page).then((v) => v.span)).toBeLessThan(before.span);
  const afterPinch = await viewport(page);
  expect(afterPinch.freq).toBeCloseTo(before.freq, 3);

  // Both panes re-rendered off the one shared viewport, so they stay locked to
  // the same time axis rather than drifting apart.
  await expect
    .poll(() => renderToken(page, 'waveform-canvas'))
    .toBeGreaterThan(waveTokenBefore);
  await expect
    .poll(() => renderToken(page, 'spectrogram-canvas'))
    .toBeGreaterThan(spectroTokenBefore);

  // Frequency / amplitude zoom now lives on Alt+wheel: it moves the ceiling and
  // leaves the time span alone.
  const beforeAlt = await viewport(page);
  await timelineWheel(page, -500, { altKey: true });
  await expect.poll(() => viewport(page).then((v) => v.freq)).not.toBe(beforeAlt.freq);
  const afterAlt = await viewport(page);
  expect(afterAlt.span).toBeCloseTo(beforeAlt.span, 3);
});
