import { expect, type Page } from '@playwright/test';

/**
 * Enters the editor for a single WAV fixture through the home screen.
 *
 * The app opens on the project manager; dropping (here, selecting) a WAV creates
 * a project, and clicking its corpus row opens that recording in the editor.
 */
export async function openEditorWithFixture(page: Page, wavPath: string): Promise<void> {
  await page.goto('/');
  await page.getByTestId('folder-input').setInputFiles([wavPath]);
  await expect(page.getByTestId('corpus')).toBeVisible();
  await page.getByTestId('corpus-row').first().click();
  await expect(page.getByTestId('editor')).toHaveAttribute('data-visible-end', /[1-9]/);
}
