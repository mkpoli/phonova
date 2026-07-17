import { expect, test, type Page } from '@playwright/test';
import fs from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { execFileSync } from 'node:child_process';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const screenshots = path.join(here, 'screenshots');
const audioDir = path.join(root, 'tests/fixtures/audio');

// Small synthetic takes keep project creation quick; each becomes its own
// single-recording project named after the WAV stem.
const SOURCES = {
  vowel: 'synth_vowel_a.wav',
  sweep: 'synth_tone_sweep.wav',
  arctic: 'arctic_bdl_a0001.wav'
} as const;

function card(page: Page, name: string) {
  return page.locator(`[data-testid="project-card"][data-project-name="${name}"]`);
}

/** Creates one project from `source`, named `name`, and returns to the home grid. */
async function createProject(page: Page, source: string, name: string) {
  await expect(page.getByTestId('home')).toBeVisible();
  await page.getByTestId('folder-input').setInputFiles({
    name: `${name}.wav`,
    mimeType: 'audio/wav',
    buffer: fs.readFileSync(path.join(audioDir, source))
  });
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(1, { timeout: 30_000 });
  await page.getByTestId('back-home').click();
  await expect(card(page, name)).toBeVisible();
}

// Pointer-based card drag: the grid moves cards through pointer events, so the
// synthesised mouse stream drives the same handlers a real drag would.
async function dragCardTo(page: Page, name: string, target: ReturnType<Page['locator']>) {
  const handle = card(page, name).getByTestId('card-drag');
  const s = await handle.boundingBox();
  const t = await target.boundingBox();
  if (!s || !t) throw new Error('missing bounding box for card drag');
  await page.mouse.move(s.x + s.width / 2, s.y + s.height / 2);
  await page.mouse.down();
  await page.mouse.move(t.x + t.width / 2, t.y + t.height / 2, { steps: 10 });
  await page.mouse.move(t.x + t.width / 2, t.y + t.height / 2, { steps: 3 });
  await page.mouse.up();
}

/** Reads a bundle's project name straight from its zipped manifest. */
function bundleName(file: string): string {
  const manifest = execFileSync('unzip', ['-p', file, 'manifest.json']);
  return JSON.parse(manifest.toString('utf8')).name as string;
}

test('pin lifts a project to the top and persists across reload', async ({ page }) => {
  await page.goto('/');
  await createProject(page, SOURCES.vowel, 'alpha');
  await createProject(page, SOURCES.sweep, 'beta');

  // Newest first: beta then alpha. Pinning alpha moves it above beta.
  await card(page, 'alpha').getByTestId('pin-project').click();

  await expect(page.getByTestId('home-pinned').locator('[data-project-name="alpha"]')).toBeVisible();
  await expect(page.locator('[data-testid="project-card"]').first()).toHaveAttribute(
    'data-project-name',
    'alpha'
  );
  await expect(card(page, 'alpha')).toHaveAttribute('data-pinned', 'true');

  // The pin is written to the home index; give the write a beat, then reload.
  await page.waitForTimeout(600);
  await page.reload();

  await expect(page.getByTestId('home-pinned').locator('[data-project-name="alpha"]')).toBeVisible();
  await expect(page.locator('[data-testid="project-card"]').first()).toHaveAttribute(
    'data-project-name',
    'alpha'
  );
});

test('group create, drag-in, and collapse round-trip across reload', async ({ page }) => {
  await page.goto('/');
  await createProject(page, SOURCES.vowel, 'alpha');
  await createProject(page, SOURCES.sweep, 'beta');

  await page.getByTestId('home-new-group').click();
  const group = page.getByTestId('home-group');
  await expect(group).toBeVisible();
  await expect(group.getByTestId('group-count')).toHaveText('0');

  await dragCardTo(page, 'beta', group);
  await expect(group.getByTestId('group-count')).toHaveText('1');
  await expect(group.locator('[data-project-name="beta"]')).toBeVisible();

  // Collapse hides the member; expanded state and membership both persist.
  await group.getByTestId('group-disclose').click();
  await expect(group.locator('[data-project-name="beta"]')).toBeHidden();

  await page.waitForTimeout(600);
  await page.reload();

  const reloaded = page.getByTestId('home-group');
  await expect(reloaded.getByTestId('group-count')).toHaveText('1');
  await expect(reloaded.getByTestId('group-disclose')).toHaveAttribute('aria-expanded', 'false');
  await expect(reloaded.locator('[data-project-name="beta"]')).toBeHidden();
});

test('modifier clicks multi-select, then a single confirm deletes the batch', async ({ page }) => {
  await page.goto('/');
  await createProject(page, SOURCES.vowel, 'alpha');
  await createProject(page, SOURCES.sweep, 'beta');
  await createProject(page, SOURCES.arctic, 'gamma');

  // Ctrl-click anchors the selection; Shift-click extends it over the run.
  await card(page, 'gamma').getByTestId('open-project').click({ modifiers: ['Control'] });
  await expect(page.getByTestId('selection-count')).toHaveText('1 selected');
  await card(page, 'alpha').getByTestId('open-project').click({ modifiers: ['Shift'] });
  await expect(page.getByTestId('selection-count')).toHaveText('3 selected');

  await page.getByTestId('batch-delete').click();
  const confirm = page.getByTestId('batch-delete-confirm');
  await expect(confirm).toBeVisible();
  await expect(confirm.locator('h2')).toHaveText('Delete 3 projects?');
  await page.getByTestId('batch-delete-confirm-action').click();

  await expect(page.getByTestId('home-empty')).toBeVisible();
});

test('batch export downloads one valid bundle per selected project', async ({ page }) => {
  await page.goto('/');
  await createProject(page, SOURCES.vowel, 'alpha');
  await createProject(page, SOURCES.sweep, 'beta');

  await card(page, 'alpha').getByTestId('open-project').click({ modifiers: ['Control'] });
  await card(page, 'beta').getByTestId('open-project').click({ modifiers: ['Control'] });
  await expect(page.getByTestId('selection-count')).toHaveText('2 selected');

  const downloads: Array<Promise<string>> = [];
  const tmp = fs.mkdtempSync(path.join(os.tmpdir(), 'phx-batch-'));
  page.on('download', (download) => {
    const dest = path.join(tmp, download.suggestedFilename());
    downloads.push(download.saveAs(dest).then(() => dest));
  });

  await page.getByTestId('batch-export').click();
  await expect.poll(() => downloads.length, { timeout: 30_000 }).toBe(2);
  const files = await Promise.all(downloads);

  // Each file is a real zip container whose manifest names its project.
  const names = files.map(bundleName).sort();
  expect(names).toEqual(['alpha', 'beta']);
  for (const file of files) {
    expect(fs.statSync(file).size).toBeGreaterThan(0);
    expect(fs.readFileSync(file).subarray(0, 2).toString('latin1')).toBe('PK');
  }

  // Selection clears once the sequential export finishes.
  await expect(page.getByTestId('selection-toolbar')).toHaveCount(0);
});

test('home renders pins, groups, and selection in both themes', async ({ page }) => {
  await page.goto('/');
  await createProject(page, SOURCES.vowel, 'alpha');
  await createProject(page, SOURCES.sweep, 'beta');
  await createProject(page, SOURCES.arctic, 'gamma');

  await card(page, 'alpha').getByTestId('pin-project').click();
  await page.getByTestId('home-new-group').click();
  await dragCardTo(page, 'beta', page.getByTestId('home-group'));
  await expect(page.getByTestId('home-group').getByTestId('group-count')).toHaveText('1');

  // Rename the group so the screenshot shows a named collection.
  await page.getByTestId('home-group').getByTestId('rename-group-edit').click();
  const rename = page.getByTestId('rename-group-input');
  await rename.fill('Field session');
  await rename.press('Enter');

  await card(page, 'gamma').getByTestId('open-project').click({ modifiers: ['Control'] });
  await expect(page.getByTestId('selection-toolbar')).toBeVisible();

  await page.screenshot({ path: path.join(screenshots, 'home-management-light.png'), fullPage: true });
  await page.getByLabel('Toggle theme').first().click();
  await expect(page.locator('html')).toHaveClass(/dark/);
  await page.screenshot({ path: path.join(screenshots, 'home-management-dark.png'), fullPage: true });
});
