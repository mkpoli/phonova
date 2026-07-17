import { expect, test, type Locator, type Page } from '@playwright/test';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const audioDir = path.join(root, 'tests/fixtures/audio');

function wav(source: string, name: string) {
  return { name, mimeType: 'audio/wav', buffer: fs.readFileSync(path.join(audioDir, source)) };
}

const twoTakes = [
  wav('synth_vowel_a.wav', 'synth_vowel_a.wav'),
  wav('synth_tone_sweep.wav', 'synth_tone_sweep.wav')
];

async function importTakes(page: Page, files = twoTakes) {
  await page.goto('/');
  await page.getByTestId('folder-input').setInputFiles(files);
  await expect(page.getByTestId('corpus')).toBeVisible();
  await expect(page.getByTestId('corpus-row')).toHaveCount(files.length, { timeout: 30_000 });
}

async function reopenProject(page: Page) {
  await page.reload();
  await expect(page.getByTestId('project-card')).toHaveCount(1);
  await page.getByTestId('open-project').first().click();
  const prompt = page.getByTestId('recovery-accept');
  if (await prompt.isVisible().catch(() => false)) await prompt.click();
  await expect(page.getByTestId('corpus')).toBeVisible();
}

/** Resolves once the corpus has persisted a write newer than `previous`. */
async function waitForPersist(page: Page, previous: string | null) {
  await expect
    .poll(() => page.getByTestId('corpus').getAttribute('data-saved-at'))
    .not.toBe(previous);
}

// Pointer-based drag: the tree uses pointer events, so the CDP-synthesised
// mouse stream drives the same handlers a real drag would.
async function dragOnto(page: Page, handle: Locator, target: Locator) {
  const s = await handle.boundingBox();
  const t = await target.boundingBox();
  if (!s || !t) throw new Error('missing bounding box for drag');
  await page.mouse.move(s.x + s.width / 2, s.y + s.height / 2);
  await page.mouse.down();
  await page.mouse.move(t.x + t.width / 2, t.y + t.height / 2, { steps: 10 });
  await page.mouse.move(t.x + t.width / 2, t.y + t.height / 2, { steps: 3 });
  await page.mouse.up();
}

function treeRowFor(page: Page, name: string): Locator {
  return page.locator('[data-testid="tree-row"]', {
    has: page.locator(`[data-recording-name="${name}"]`)
  });
}

test('rename a corpus row from the keyboard and it survives reload', async ({ page }) => {
  await importTakes(page);

  const name = page.locator('[data-recording-name="synth_vowel_a"]').getByTestId('rename-corpus-name');
  await name.focus();
  await page.keyboard.press('F2');
  const input = page.getByTestId('rename-corpus-input');
  await expect(input).toBeVisible();
  await input.fill('vowel_renamed');
  await page.keyboard.press('Enter');

  await expect(page.locator('[data-recording-name="vowel_renamed"]')).toHaveCount(1);

  await reopenProject(page);
  await expect(page.locator('[data-recording-name="vowel_renamed"]')).toHaveCount(1);
});

test('create a group, drag a recording into it, and the nesting round-trips', async ({ page }) => {
  await importTakes(page);

  const savedOnImport = await page.getByTestId('corpus').getAttribute('data-saved-at');
  await page.getByTestId('new-group').click();
  const groupRow = page.locator('[data-testid="tree-row"]', {
    has: page.getByTestId('rename-group-name')
  });
  await expect(groupRow).toHaveCount(1);
  await expect(groupRow.getByTestId('group-count')).toHaveText('0');
  // Let the group-creation write flush before recording the next baseline.
  await waitForPersist(page, savedOnImport);
  const savedAfterGroup = await page.getByTestId('corpus').getAttribute('data-saved-at');

  // Drag the sweep recording onto the group to nest it.
  const handle = treeRowFor(page, 'synth_tone_sweep').getByTestId('tree-drag');
  await dragOnto(page, handle, groupRow);

  await expect(groupRow.getByTestId('group-count')).toHaveText('1');
  await expect(treeRowFor(page, 'synth_tone_sweep')).toHaveAttribute('data-depth', '1');

  // The move persists asynchronously; wait for its container write to flush.
  await waitForPersist(page, savedAfterGroup);
  await reopenProject(page);

  const groupRowAfter = page.locator('[data-testid="tree-row"]', {
    has: page.getByTestId('rename-group-name')
  });
  await expect(groupRowAfter.getByTestId('group-count')).toHaveText('1');
  await expect(treeRowFor(page, 'synth_tone_sweep')).toHaveAttribute('data-depth', '1');
});

test('delete a recording, then undo restores the row', async ({ page }) => {
  await importTakes(page);

  await treeRowFor(page, 'synth_tone_sweep').getByTestId('row-delete').click();
  await expect(page.getByTestId('corpus-row')).toHaveCount(1);
  await expect(page.locator('[data-recording-name="synth_tone_sweep"]')).toHaveCount(0);

  await expect(page.getByTestId('removal-undo')).toBeVisible();
  await page.getByTestId('removal-undo-action').click();

  await expect(page.getByTestId('corpus-row')).toHaveCount(2);
  await expect(page.locator('[data-recording-name="synth_tone_sweep"]')).toHaveCount(1);
});

test('search narrows the corpus by tag', async ({ page }) => {
  await importTakes(page);

  // Tag the vowel recording through its details panel.
  await treeRowFor(page, 'synth_vowel_a').getByTestId('row-details').click();
  const tagInput = page.getByTestId('metadata-tag-input');
  await tagInput.fill('fieldwork');
  await page.keyboard.press('Enter');
  await expect(page.getByTestId('metadata-tags')).toContainText('fieldwork');

  await page.getByTestId('corpus-search').fill('fieldwork');
  await expect(page.getByTestId('corpus-row')).toHaveCount(1);
  await expect(page.locator('[data-recording-name="synth_vowel_a"]')).toHaveCount(1);

  // A name query narrows to the other recording.
  await page.getByTestId('corpus-search').fill('sweep');
  await expect(page.getByTestId('corpus-row')).toHaveCount(1);
  await expect(page.locator('[data-recording-name="synth_tone_sweep"]')).toHaveCount(1);

  // Clearing the query restores the full corpus.
  await page.getByTestId('corpus-search').fill('');
  await expect(page.getByTestId('corpus-row')).toHaveCount(2);
});
