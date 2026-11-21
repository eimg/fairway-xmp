import { test, expect } from '@playwright/test';
import { nativeBridge } from './native-bridge';

// Use the same production frontend that is packaged in the desktop app.
test.use({ baseURL: 'http://127.0.0.1:1422' });

test('desktop data browser starts bundled MySQL and edits real rows', async ({ page }, info) => {
  test.setTimeout(120000);
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/');
    await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
    await page.getByRole('button', { name: 'Start MySQL', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Database', exact: true })).toBeEnabled({ timeout: 60000 });
    await page.getByRole('button', { name: 'Database', exact: true }).click();
    let dialog = page.getByRole('dialog', { name: 'Create database', exact: true });
    await dialog.getByLabel('Database name', { exact: true }).fill('native_classroom');
    await dialog.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(page.getByLabel('Database', { exact: true })).toHaveValue('native_classroom');
    await page.getByRole('button', { name: 'Create table', exact: true }).first().click();
    dialog = page.getByRole('dialog', { name: 'Create table', exact: true });
    await dialog.getByLabel('Table name', { exact: true }).fill('students');
    await dialog.getByRole('button', { name: 'Create', exact: true }).click();
    await expect(dialog).toHaveCount(0);
    await page.getByRole('button', { name: 'Insert row', exact: true }).click();
    dialog = page.getByRole('dialog', { name: 'Insert row', exact: true });
    await expect(dialog.getByLabel('name value', { exact: true })).toBeEnabled();
    await dialog.getByLabel('name value', { exact: true }).fill("Student May's 日本");
    await dialog.getByRole('button', { name: 'Save', exact: true }).click();
    const rows = page.getByRole('region', { name: 'Table rows', exact: true });
    await expect(rows).toContainText("Student May's 日本");
    await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
    dialog = page.getByRole('dialog', { name: 'Edit row', exact: true });
    await dialog.getByLabel('name value', { exact: true }).fill('Edited through native PHP');
    await dialog.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(rows).toContainText('Edited through native PHP');
    await page.getByRole('tab', { name: 'SQL query', exact: true }).click();
    await page.locator('#db-sql').fill('SELECT name FROM students');
    await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
    await expect(page.getByRole('region', { name: 'Query result 1', exact: true })).toContainText('Edited through native PHP');
    expect(bridge.errors()).toBe('');
  } finally {
    await bridge.close();
  }
});
