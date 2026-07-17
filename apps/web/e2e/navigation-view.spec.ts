import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { openEditorWithFixture } from './helpers';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const vowelFixture = path.join(root, 'tests/fixtures/audio/synth_vowel_a.wav');
const arcticFixture = path.join(root, 'tests/fixtures/audio/arctic_bdl_a0001.wav');
const textGridFixture = path.join(root, 'tests/fixtures/textgrids/arctic_bdl_a0001_long_utf8.TextGrid');
const screenshots = path.join(here, 'screenshots');

function editor(page: Page) {
  return page.getByTestId('editor');
}

function visibleStart(page: Page) {
  return editor(page).getAttribute('data-visible-start').then(Number);
}
function visibleEnd(page: Page) {
  return editor(page).getAttribute('data-visible-end').then(Number);
}
function visibleFreq(page: Page) {
  return editor(page).getAttribute('data-visible-freq').then(Number);
}
function cursorTime(page: Page) {
  return editor(page).getAttribute('data-cursor-time').then(Number);
}

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}

/** Drags a time–frequency box across the spectrogram; returns its signal coords. */
async function dragSpectrogramBox(page: Page, rel = { x0: 0.3, x1: 0.7, y0: 0.3, y1: 0.7 }) {
  const canvas = page.getByTestId('spectrogram-canvas');
  const box = (await canvas.boundingBox()) as Box;
  const x0 = box.x + box.width * rel.x0;
  const x1 = box.x + box.width * rel.x1;
  const y0 = box.y + box.height * rel.y0;
  const y1 = box.y + box.height * rel.y1;
  await page.mouse.move(x0, y0);
  await page.mouse.down();
  await page.mouse.move((x0 + x1) / 2, (y0 + y1) / 2, { steps: 4 });
  await page.mouse.move(x1, y1, { steps: 4 });
  await page.mouse.up();
  const bar = page.getByTestId('readout-bar');
  await expect(bar).toBeVisible();
  return {
    t0: Number(await bar.getAttribute('data-t0')),
    t1: Number(await bar.getAttribute('data-t1')),
    f0: Number(await bar.getAttribute('data-f0')),
    f1: Number(await bar.getAttribute('data-f1'))
  };
}

test('gating: waveform pane background is dark in dark theme', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  await page.getByRole('button', { name: 'Toggle theme' }).click();
  await expect
    .poll(() => page.evaluate(() => document.documentElement.classList.contains('dark')))
    .toBe(true);
  // Let the theme-change redraw land.
  await page.waitForTimeout(400);

  // Sample the dominant (background) colour of the waveform canvas: it must be
  // the charcoal canvas, not the light paper it regressed to.
  const luma = await page.getByTestId('waveform-canvas').evaluate(async (canvas: HTMLCanvasElement) => {
    const bmp = await createImageBitmap(canvas);
    const off = new OffscreenCanvas(bmp.width, bmp.height);
    const ctx = off.getContext('2d')!;
    ctx.drawImage(bmp, 0, 0);
    const data = ctx.getImageData(0, 0, bmp.width, bmp.height).data;
    const tally = new Map<number, { n: number; r: number; g: number; b: number }>();
    for (let i = 0; i < data.length; i += 4) {
      const key = (data[i] << 16) | (data[i + 1] << 8) | data[i + 2];
      const e = tally.get(key) ?? { n: 0, r: data[i], g: data[i + 1], b: data[i + 2] };
      e.n += 1;
      tally.set(key, e);
    }
    let best = { n: 0, r: 0, g: 0, b: 0 };
    for (const e of tally.values()) if (e.n > best.n) best = e;
    return 0.2126 * best.r + 0.7152 * best.g + 0.0722 * best.b;
  });
  // Charcoal canvas (~#131210) is far below mid-grey; the old light paper was ~240.
  expect(luma).toBeLessThan(60);
});

test('double-click inside a selection zooms to it; empty space fits the file', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  const fileEnd = await visibleEnd(page);

  const coords = await dragSpectrogramBox(page);
  expect(coords.t1).toBeGreaterThan(coords.t0);

  // Double-click inside the box zooms the viewport onto the box's time span.
  const canvas = page.getByTestId('spectrogram-canvas');
  const cbox = (await canvas.boundingBox()) as Box;
  await page.mouse.dblclick(cbox.x + cbox.width * 0.5, cbox.y + cbox.height * 0.5);

  await expect.poll(() => visibleStart(page)).toBeGreaterThan(coords.t0 - 0.03);
  await expect.poll(() => visibleEnd(page)).toBeLessThan(coords.t1 + 0.03);
  const zoomedSpan = (await visibleEnd(page)) - (await visibleStart(page));
  expect(zoomedSpan).toBeLessThan(fileEnd);

  // Double-click on empty space (no selection) fits the whole file back.
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('readout-bar')).toHaveCount(0);
  await page.mouse.dblclick(cbox.x + cbox.width * 0.5, cbox.y + cbox.height * 0.5);
  await expect.poll(() => visibleStart(page)).toBeLessThan(0.01);
  await expect.poll(() => visibleEnd(page)).toBeGreaterThan(fileEnd - 0.01);
});

test('transport Space plays the selection span, then the visible span when zoomed', async ({
  page
}) => {
  await openEditorWithFixture(page, vowelFixture);

  // A box selection: Space seeks to its start and plays only its span.
  const coords = await dragSpectrogramBox(page, { x0: 0.4, x1: 0.62, y0: 0.2, y1: 0.8 });
  expect(coords.t0).toBeGreaterThan(0.01);
  await page.keyboard.press('Space');
  await expect.poll(() => cursorTime(page)).toBeGreaterThanOrEqual(coords.t0 - 1e-3);
  // Playback is bounded by the span: after it ends the cursor sits at the span
  // end, never running on to the file end.
  await page.waitForTimeout(Math.max(400, (coords.t1 - coords.t0) * 1000 + 400));
  const afterSel = await cursorTime(page);
  expect(afterSel).toBeLessThanOrEqual(coords.t1 + 0.06);

  // Clear the selection, zoom into a window that does not start at 0, and Space
  // plays exactly that visible window (cursor jumps to the window start).
  await page.keyboard.press('Escape');
  await expect(page.getByTestId('readout-bar')).toHaveCount(0);
  const canvas = page.getByTestId('spectrogram-canvas');
  const cbox = (await canvas.boundingBox()) as Box;
  await page.mouse.move(cbox.x + cbox.width * 0.7, cbox.y + cbox.height * 0.5);
  for (let i = 0; i < 6; i += 1) await page.mouse.wheel(0, -120);
  const winStart = await visibleStart(page);
  expect(winStart).toBeGreaterThan(0.02);
  await page.keyboard.press('Space');
  await expect.poll(() => cursorTime(page)).toBeGreaterThanOrEqual(winStart - 1e-3);
});

test('clicking a tier interval selects and plays it, and never opens the label editor', async ({
  page
}) => {
  await openEditorWithFixture(page, arcticFixture);
  await expect(page.getByTestId('tier-pane')).toHaveAttribute('data-undo-depth', '1', {
    timeout: 60_000
  });
  // Import a TextGrid so the pane holds real labelled intervals.
  await page.getByTestId('textgrid-input').setInputFiles(textGridFixture);
  await expect(page.getByTestId('interval').first()).toBeVisible({ timeout: 30_000 });

  // Click an interval past the origin.
  const intervals = page.getByTestId('interval');
  const count = await intervals.count();
  const target = intervals.nth(Math.min(3, count - 1));
  const xmin = Number(await target.getAttribute('data-xmin'));
  await target.click();

  // The click sets the time selection (readout bar appears) and plays it (cursor
  // advances from the interval start), and it does not open the label editor.
  await expect(page.getByTestId('readout-bar')).toBeVisible();
  await expect(page.getByTestId('label-editor')).toHaveCount(0);
  await expect.poll(() => cursorTime(page)).toBeGreaterThanOrEqual(xmin - 1e-3);

  // Double-click still opens the editor — the click/edit split holds.
  await target.dblclick();
  await expect(page.getByTestId('label-editor')).toBeVisible();
});

test('box-selection play renders through the band filter, and is banded numerically', async ({
  page
}) => {
  await openEditorWithFixture(page, vowelFixture);

  // A low-frequency box over the sustained vowel: its energy concentrates low.
  const coords = await dragSpectrogramBox(page, { x0: 0.3, x1: 0.7, y0: 0.7, y1: 0.95 });
  expect(coords.f1).toBeGreaterThan(coords.f0);

  // The play affordance is discoverable: for a box it states the band.
  await expect(page.getByTestId('selection-play')).toContainText('Play band');

  // Clicking it renders and sounds the filtered buffer, showing the indicator.
  await page.getByTestId('selection-play').click();
  await expect(page.getByTestId('filtered-indicator')).toBeVisible();

  // Numerically banded: the engine's band render for the selected low band
  // carries more energy than a render of an equal-width high band with little
  // signal, over the same span.
  const rms = await page.evaluate(async (c) => {
    const hook = (globalThis as unknown as { __phonia?: { client: any; audioId: bigint | null } })
      .__phonia;
    if (!hook || hook.audioId === null) throw new Error('no client hook');
    const energy = (buf: Float32Array) => {
      let sum = 0;
      for (let i = 0; i < buf.length; i += 1) sum += buf[i] * buf[i];
      return buf.length ? Math.sqrt(sum / buf.length) : 0;
    };
    const inBand = await hook.client.bandFilteredSpan(hook.audioId, c.t0, c.t1, c.f0, c.f1);
    const width = c.f1 - c.f0;
    const outBand = await hook.client.bandFilteredSpan(hook.audioId, c.t0, c.t1, 5500, 5500 + width);
    return { inBand: energy(inBand as Float32Array), outBand: energy(outBand as Float32Array) };
  }, coords);
  expect(rms.inBand).toBeGreaterThan(0);
  expect(rms.inBand).toBeGreaterThan(rms.outBand * 2);
});

test('frequency-ruler drag scales the ceiling; the reset chip restores it', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  await expect.poll(() => visibleFreq(page)).toBeGreaterThan(4999);

  const ruler = page.getByTestId('frequency-ruler');
  const rbox = (await ruler.boundingBox()) as Box;
  // Drag down the ruler to lower the ceiling.
  await page.mouse.move(rbox.x + rbox.width / 2, rbox.y + rbox.height * 0.35);
  await page.mouse.down();
  await page.mouse.move(rbox.x + rbox.width / 2, rbox.y + rbox.height * 0.75, { steps: 6 });
  await page.mouse.up();

  await expect.poll(() => visibleFreq(page)).toBeLessThan(4900);
  const reset = page.getByTestId('frequency-reset');
  await expect(reset).toBeVisible();
  await reset.click();
  await expect.poll(() => visibleFreq(page)).toBeGreaterThan(4999);
  await expect(page.getByTestId('frequency-reset')).toHaveCount(0);
});

test('waveform amplitude gutter scales gain; the reset chip restores it', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  const gutter = page.getByTestId('amp-gutter');
  const gbox = (await gutter.boundingBox()) as Box;
  await page.mouse.move(gbox.x + gbox.width / 2, gbox.y + gbox.height * 0.7);
  await page.mouse.down();
  await page.mouse.move(gbox.x + gbox.width / 2, gbox.y + gbox.height * 0.2, { steps: 6 });
  await page.mouse.up();
  const reset = page.getByTestId('amp-reset');
  await expect(reset).toBeVisible();
  await reset.click();
  await expect(page.getByTestId('amp-reset')).toHaveCount(0);
});

test('waveform toggle hides the pane and ghosts the envelope over the spectrogram', async ({
  page
}) => {
  await openEditorWithFixture(page, vowelFixture);
  await expect(page.getByTestId('waveform-canvas')).toBeVisible();
  await expect(page.getByTestId('ghost-waveform')).toHaveCount(0);

  await page.keyboard.press('w');
  await expect(page.getByTestId('waveform-canvas')).toHaveCount(0);
  await expect(page.getByTestId('ghost-waveform')).toBeVisible();
  await expect(page.getByTestId('timeline')).toHaveAttribute('data-waveform-visible', 'false');

  await page.keyboard.press('w');
  await expect(page.getByTestId('waveform-canvas')).toBeVisible();
  await expect(page.getByTestId('ghost-waveform')).toHaveCount(0);
});

test('waveform level of detail switches to raw samples and dots at deep zoom', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  const wave = page.getByTestId('waveform-canvas');
  await expect(wave).toHaveAttribute('data-lod', 'envelope');

  const canvas = page.getByTestId('spectrogram-canvas');
  const cbox = (await canvas.boundingBox()) as Box;
  await page.mouse.move(cbox.x + cbox.width * 0.5, cbox.y + cbox.height * 0.5);
  for (let i = 0; i < 40; i += 1) await page.mouse.wheel(0, -120);

  await expect(wave).toHaveAttribute('data-lod', /raw/, { timeout: 15_000 });

  // Column-variance detail: a sample-accurate raw draw fills more distinct
  // vertical positions than a flat envelope of the same view would.
  const variance = await wave.evaluate(async (canvasEl: HTMLCanvasElement) => {
    const bmp = await createImageBitmap(canvasEl);
    const off = new OffscreenCanvas(bmp.width, bmp.height);
    const ctx = off.getContext('2d')!;
    ctx.drawImage(bmp, 0, 0);
    const { width, height, data } = ctx.getImageData(0, 0, bmp.width, bmp.height);
    let painted = 0;
    for (let x = 0; x < width; x += 1) {
      const seen = new Set<number>();
      for (let y = 0; y < height; y += 1) {
        const i = (y * width + x) * 4;
        // Any non-background (non-dark, non-flat) pixel counts as painted content.
        if (data[i] + data[i + 1] + data[i + 2] > 120) seen.add(y);
      }
      painted += seen.size;
    }
    return painted / width;
  });
  // A raw polyline through samples paints multiple rows per column on average.
  expect(variance).toBeGreaterThan(1.5);
});

test('UI scale adjusts the root font size and persists across reload', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);
  const rootFontPx = () =>
    page.evaluate(() => parseFloat(getComputedStyle(document.documentElement).fontSize));

  expect(await rootFontPx()).toBeCloseTo(16, 0);
  await page.keyboard.press('Control+Equal');
  await page.keyboard.press('Control+Equal');
  await expect.poll(rootFontPx).toBeGreaterThan(18);
  const scaled = await rootFontPx();

  await page.reload();
  await expect.poll(rootFontPx).toBeCloseTo(scaled, 0);

  // Reset restores the base size and clears persistence.
  await page.keyboard.press('Control+Digit0');
  await expect.poll(rootFontPx).toBeCloseTo(16, 0);
});

test('navigation view: light and dark screenshots of the new states', async ({ page }) => {
  await openEditorWithFixture(page, vowelFixture);

  // Scaled vertical state with both reset chips, light: drag the frequency ruler
  // down and the amplitude gutter up.
  const ruler = page.getByTestId('frequency-ruler');
  const rbox = (await ruler.boundingBox()) as Box;
  await page.mouse.move(rbox.x + rbox.width / 2, rbox.y + rbox.height * 0.35);
  await page.mouse.down();
  await page.mouse.move(rbox.x + rbox.width / 2, rbox.y + rbox.height * 0.7, { steps: 6 });
  await page.mouse.up();
  const gutter = page.getByTestId('amp-gutter');
  const gbox = (await gutter.boundingBox()) as Box;
  await page.mouse.move(gbox.x + gbox.width / 2, gbox.y + gbox.height * 0.7);
  await page.mouse.down();
  await page.mouse.move(gbox.x + gbox.width / 2, gbox.y + gbox.height * 0.25, { steps: 6 });
  await page.mouse.up();
  await expect(page.getByTestId('frequency-reset')).toBeVisible();
  await expect(page.getByTestId('amp-reset')).toBeVisible();
  // Let the transform-first draw settle onto the fresh backing store before the
  // capture, so the amplified waveform is on the canvas, not mid-transform.
  await page.waitForTimeout(700);
  await page.screenshot({ path: path.join(screenshots, 'vertical-scale-light.png'), fullPage: true });

  // Ghost overlay mode, dark.
  await page.getByTestId('frequency-reset').click();
  await page.keyboard.press('w');
  await expect(page.getByTestId('ghost-waveform')).toBeVisible();
  await page.getByRole('button', { name: 'Toggle theme' }).click();
  await expect
    .poll(() => page.evaluate(() => document.documentElement.classList.contains('dark')))
    .toBe(true);
  await page.waitForTimeout(400);
  await page.screenshot({ path: path.join(screenshots, 'ghost-overlay-dark.png'), fullPage: true });
});
