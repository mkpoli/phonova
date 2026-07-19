import { expect, test, type Page } from '@playwright/test';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const here = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(here, '../../..');
const wavFixture = path.join(root, 'tests/fixtures/audio/arctic_bdl_a0001.wav');

// `?app=1` is the landing page's own bypass (see routes/+page.svelte,
// computeShowLanding) — used here instead of the shared `openEditorWithFixture`
// helper so this spec does not depend on the visited-flag/landing-page path
// other in-flight work in this repo is actively touching.
async function openEditorWithFixture(page: Page): Promise<void> {
  await page.goto('/?app=1');
  await page.getByTestId('folder-input').setInputFiles([wavFixture]);
  await expect(page.getByTestId('corpus')).toBeVisible();
  await page.getByTestId('corpus-row').first().click();
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);
}

/** Pre-answers the first-run prompt so a test can focus on its own concern. */
async function seedKeyMode(page: Page, mode: 'phonia' | 'praat'): Promise<void> {
  await page.addInitScript((value) => localStorage.setItem('phonia:key-mode', value), mode);
}

function shortcutEditor(page: Page) {
  return page.getByTestId('shortcut-editor');
}

// Opens the shortcut editor through the command palette, which — unlike the
// Home-screen gear button — is reachable from any screen, including the
// editor these tests are otherwise sitting in.
async function setKeyMode(page: Page, mode: 'phonia' | 'praat'): Promise<void> {
  await page.keyboard.press('Control+k');
  const palette = page.getByTestId('command-palette');
  await expect(palette).toBeVisible();
  await page.getByTestId('command-palette-input').fill('keyboard shortcuts');
  const item = page.locator('[data-testid="command-item"][data-command-id="openShortcutEditor"]');
  await expect(item).toBeVisible();
  await item.click();
  await expect(palette).toHaveCount(0);
  await expect(shortcutEditor(page)).toBeVisible();
  await page.getByTestId(`shortcut-mode-${mode}`).click();
  await page.getByTestId('shortcut-editor-close').click();
  await expect(shortcutEditor(page)).toHaveCount(0);
}

test('first-run key-mode prompt appears once, then never again', async ({ page }) => {
  await page.goto('/?app=1');
  await expect(page.getByTestId('home')).toBeVisible();

  const prompt = page.getByTestId('key-mode-prompt');
  await expect(prompt).toBeVisible();

  await page.getByTestId('key-mode-choice-phonia').click();
  await expect(prompt).toHaveCount(0);
  expect(await page.evaluate(() => localStorage.getItem('phonia:key-mode'))).toBe('phonia');

  // A fresh load after answering must never show the prompt again.
  await page.reload();
  await expect(page.getByTestId('home')).toBeVisible();
  await expect(prompt).toHaveCount(0);
});

test('dismissing the prompt without choosing still counts as answered', async ({ page }) => {
  await page.goto('/?app=1');
  const prompt = page.getByTestId('key-mode-prompt');
  await expect(prompt).toBeVisible();

  await page.getByTestId('key-mode-prompt-later').click();
  await expect(prompt).toHaveCount(0);

  await page.reload();
  await expect(page.getByTestId('home')).toBeVisible();
  await expect(prompt).toHaveCount(0);
});

test('key mode changes what Tab does in the editor', async ({ page }) => {
  await seedKeyMode(page, 'phonia');
  await openEditorWithFixture(page);
  const editor = page.getByTestId('editor');

  // Phonia mode: Tab has no editor-scope binding, so it must not start
  // playback — the cursor stays put.
  const beforePhonia = Number(await editor.getAttribute('data-cursor-time'));
  await page.keyboard.press('Tab');
  await page.waitForTimeout(400);
  const afterPhonia = Number(await editor.getAttribute('data-cursor-time'));
  expect(afterPhonia).toBeCloseTo(beforePhonia, 3);

  // Switch to Praat-compatible mode via the shortcut editor.
  await setKeyMode(page, 'praat');

  // Praat mode: Tab plays (same action as Space) — the engine-clock cursor
  // advances while it plays, then Tab again stops it, matching Praat's own
  // "Tab is a play/stop toggle" behavior (docs/research/praat-shortcuts.md).
  const beforePraat = Number(await editor.getAttribute('data-cursor-time'));
  await page.keyboard.press('Tab');
  await page.waitForTimeout(600);
  await page.keyboard.press('Tab');
  const afterPraat = Number(await editor.getAttribute('data-cursor-time'));
  expect(afterPraat).toBeGreaterThan(beforePraat);
});

test('Praat mode: Tab plays even when the tier pane has focus', async ({ page }) => {
  await seedKeyMode(page, 'praat');
  await openEditorWithFixture(page);
  const editor = page.getByTestId('editor');

  // Focusing the tier pane hands Tab to TierPane's own scope first; Praat
  // mode leaves that scope's Tab binding empty, so it must fall back to the
  // editor's play command through the shared command registry.
  await page.getByTestId('tier-pane').click();

  const before = Number(await editor.getAttribute('data-cursor-time'));
  await page.keyboard.press('Tab');
  await page.waitForTimeout(600);
  await page.keyboard.press('Tab');
  const after = Number(await editor.getAttribute('data-cursor-time'));
  expect(after).toBeGreaterThan(before);
});

test('rebinding a shortcut persists across reload', async ({ page }) => {
  await seedKeyMode(page, 'phonia');
  await page.goto('/?app=1');
  await expect(page.getByTestId('home')).toBeVisible();

  await page.getByTestId('open-shortcut-editor').click();
  await expect(shortcutEditor(page)).toBeVisible();

  await page.getByTestId('shortcut-search').fill('Split interval');
  const row = page.locator('[data-testid="shortcut-command"][data-command-id="insertBoundary"]');
  await expect(row).toBeVisible();
  await row.click();

  await page.getByTestId('shortcut-rebind').click();
  await expect(page.getByTestId('shortcut-listening')).toBeVisible();
  await page.keyboard.press('j');

  const detail = page.getByTestId('shortcut-detail');
  await expect(detail).toContainText('J');
  // A fresh, previously-unbound letter must not collide with anything.
  await expect(page.getByTestId('shortcut-conflicts')).toHaveCount(0);

  await page.getByTestId('shortcut-editor-close').click();
  await page.reload();

  await page.getByTestId('open-shortcut-editor').click();
  await expect(shortcutEditor(page)).toBeVisible();
  await page.getByTestId('shortcut-search').fill('Split interval');
  await expect(row).toBeVisible();
  await row.click();
  await expect(detail).toContainText('J');
});
