import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { canvasForegroundCoverage } from './helpers';

const here = path.dirname(fileURLToPath(import.meta.url));
const screenshots = path.join(here, 'screenshots');

// Capture needs a microphone; Chromium's fake device stands in with a tone, and
// the fake UI auto-grants permission. No-user-gesture autoplay keeps the audio
// graph from stalling behind a gesture the async start flow has spent.
test.use({
  launchOptions: {
    args: [
      '--use-fake-device-for-media-stream',
      '--use-fake-ui-for-media-stream',
      '--autoplay-policy=no-user-gesture-required'
    ]
  }
});

test('record from the microphone, land a corpus row, open it in the editor', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/');

  // The Record control sits beside the file actions on the home screen.
  await expect(page.getByTestId('record')).toBeVisible();
  await page.getByTestId('record').click();

  // Recording opens a non-modal strip with a live meter, elapsed time, and rate.
  const strip = page.getByTestId('recording-strip');
  await expect(strip).toBeVisible();
  await expect
    .poll(() => page.getByTestId('recording-samplerate').textContent())
    .not.toBe('—');

  // Recording from Home made a project from the timestamp; the strip names it.
  const destination = page.getByTestId('recording-destination');
  await expect(destination).toBeVisible();
  await expect(destination).toContainText('Recording into new project');
  const destinationName = (await page.getByTestId('recording-destination-name').textContent())?.trim();
  expect(destinationName).toMatch(/^Recordings \d{4}-\d{2}-\d{2}/);

  // The take is already landing in that project's corpus while capture runs.
  await expect(page.getByTestId('corpus')).toBeVisible();

  // The fake device feeds a tone, so the meter registers a non-zero level.
  await expect.poll(() => meterFill(page), { timeout: 15_000 }).toBeGreaterThan(0);
  await expect.poll(() => elapsed(page)).toBeGreaterThan(0.4);

  // Mid-recording screenshots of the strip in both themes.
  await page.screenshot({ path: path.join(screenshots, 'recording-light.png'), fullPage: true });
  await page.getByLabel('Toggle theme').click();
  await expect(page.locator('html')).toHaveClass(/dark/);
  await page.screenshot({ path: path.join(screenshots, 'recording-dark.png'), fullPage: true });
  // Back to light so the take opens in the theme the other screenshots use.
  await page.getByLabel('Toggle theme').click();
  await expect(page.locator('html')).not.toHaveClass(/dark/);

  // Capture about two seconds, then stop.
  await expect.poll(() => elapsed(page), { timeout: 15_000 }).toBeGreaterThan(2);
  await page.getByTestId('recording-stop').click();

  // Stopping opens the take in the editor.
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);
  const duration = await visibleEnd(page);
  expect(duration).toBeGreaterThan(1.5);

  // The waveform draws a real, non-empty signal. The editor also fires its
  // overlay analyses on the take; Chromium's fake tone is a worst case for the
  // pitch tracker, so the shared worker takes several seconds to reach the
  // waveform request — hence the generous window here.
  await expect(page.getByTestId('waveform-canvas')).toHaveAttribute('data-render-token', /[1-9]/, {
    timeout: 60_000
  });
  // The first render token can precede the recorded-audio paint (the canvas is
  // still blank), so poll until the take's waveform actually covers the pane
  // rather than sampling a single frame that races the paint.
  await expect
    .poll(() => canvasForegroundCoverage(page, 'waveform-canvas'), { timeout: 60_000 })
    .toBeGreaterThan(200);

  // The take is a corpus row: back out and confirm exactly one recording.
  await page.getByTestId('back-corpus').click();
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(1);
});

function elapsed(page: Page) {
  return page.getByTestId('recording-elapsed').evaluate((node) => {
    const text = node.textContent ?? '0';
    if (text.includes(':')) {
      const [minutes, rest] = text.split(':');
      return Number(minutes) * 60 + Number(rest);
    }
    return Number(text);
  });
}

function meterFill(page: Page) {
  return page
    .getByTestId('recording-level')
    .locator('.meter-rms')
    .evaluate((node) => parseFloat((node as HTMLElement).style.width) || 0);
}

function visibleEnd(page: Page) {
  return page.getByTestId('editor').evaluate((node) => Number(node.getAttribute('data-visible-end')));
}
