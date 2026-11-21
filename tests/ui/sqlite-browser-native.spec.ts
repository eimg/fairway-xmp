import { expect, test } from '@playwright/test';
import { nativeBridge } from './native-bridge';
import { readFile } from 'node:fs/promises';
import type { Snapshot } from '../../src/types';
import { join } from 'node:path';

test.use({ baseURL: 'http://127.0.0.1:1422' });

test('SQLite edits the original project file through the native worker with MySQL stopped', async ({ page }, info) => {
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/');
    await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
    await page.getByRole('button', { name: 'Open SQLite file…', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Create table', exact: true }).first()).toBeEnabled();
    await page.getByRole('tab', { name: 'SQL query', exact: true }).click();
    await page.locator('#db-sql').fill("CREATE TABLE students(id INTEGER PRIMARY KEY, name TEXT); INSERT INTO students VALUES(9007199254740993, 'Student 日本'); SELECT * FROM students;");
    await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
    await expect(page.getByRole('region', { name: 'Query result 3', exact: true })).toContainText('9007199254740993');
    await page.getByRole('tab', { name: 'Rows', exact: true }).click();
    await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
    const dialog = page.getByRole('dialog', { name: 'Edit row', exact: true });
    await dialog.getByLabel('name value', { exact: true }).fill('Edited in Fairway');
    await dialog.getByRole('button', { name: 'Save', exact: true }).click();
    await expect(page.getByRole('region', { name: 'Table rows', exact: true })).toContainText('Edited in Fairway');
    // Native layout resolves this under the isolated home; inspect the selected path.
    const selectedFile = await page.getByLabel('SQLite file path', { exact: true }).innerText();
    expect((await readFile(selectedFile)).subarray(0, 16).toString()).toBe('SQLite format 3\0');
    expect(selectedFile.replaceAll('\\', '/')).toBe(join(bridge.root, "Student May's home 日本/fairway/www/Project with spaces 日本/database.sqlite").replaceAll('\\', '/'));
    const snapshot = await page.evaluate(() => window.__TAURI_INTERNALS__!.invoke<Snapshot>('snapshot'));
    expect(snapshot.services.find(service => service.id === 'mysql')?.running).toBe(false);
    // Query the explicit original path independently of the UI's selected source.
    const saved = await page.evaluate(path => window.__TAURI_INTERNALS__!.invoke<{ sets: { rows: string[][] }[] }>('data_browser', {
      id: crypto.randomUUID(), request: { driver: 'sqlite', path, op: 'query', sql: 'SELECT name FROM students' },
    }), selectedFile);
    expect(saved.sets[0].rows).toEqual([['Edited in Fairway']]);
    expect(bridge.errors()).toBe('');
  } finally { await bridge.close(); }
});
