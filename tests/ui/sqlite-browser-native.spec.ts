import { expect, test } from '@playwright/test';
import { nativeBridge } from './native-bridge';
import { readFile, unlink } from 'node:fs/promises';
import { join } from 'node:path';

test.use({ baseURL: 'http://127.0.0.1:1422' });

for (const driver of ['mysql', 'sqlite'] as const) test(`${driver}: an unreadable first view leaves the catalog and SQL repair tools available`, async ({ page }, info) => {
  test.setTimeout(120000);
  test.skip(!process.env.FAIRWAY_TEST_RUNTIME, 'Set FAIRWAY_TEST_RUNTIME and build terminal_bridge.');
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/');
    const path = join(bridge.root, "Student May's home 日本/fairway/www/Project with spaces 日本/database.sqlite");
    await page.evaluate(async ({ path, driver }) => {
      const invoke = window.__TAURI_INTERNALS__!.invoke;
      if (driver === 'mysql') {
        await invoke('service_action', { action: 'start', service: 'mysql' });
        await invoke('data_browser', { id: crypto.randomUUID(), request: { op: 'create_database', database: 'recovery' } });
      }
      const sql = `${driver === 'mysql' ? 'CREATE TABLE missing_table(id INT); ' : ''}CREATE VIEW a_broken AS SELECT * FROM missing_table; CREATE TABLE healthy(id INTEGER PRIMARY KEY, value TEXT); INSERT INTO healthy VALUES(1,'available');${driver === 'mysql' ? ' DROP TABLE missing_table;' : ''}`;
      await invoke('data_browser', { id: crypto.randomUUID(), request: { driver, path, database: 'recovery', op: 'query', sql } });
    }, { path, driver });
    // Fixture seeding bypasses the production service action/event handler.
    // Read its resulting status on mount before driving the actual UI.
    await page.reload();
    await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
    if (driver === 'sqlite') {
      await page.getByRole('button', { name: 'Open SQLite file…', exact: true }).click();
      await expect(page.getByRole('tab', { name: 'SQLite', exact: true })).toHaveAttribute('aria-selected', 'true');
      const shown = await page.getByLabel('SQLite file path', { exact: true }).innerText();
      expect(shown.replaceAll('\\', '/')).toBe(path.replaceAll('\\', '/'));
    } else await page.getByLabel('Database', { exact: true }).selectOption('recovery');
    await expect(page.getByRole('alert')).toContainText('Could not load a_broken.');
    await expect(page.getByRole('button', { name: 'Insert row', exact: true })).toBeDisabled();
    await expect(page.getByRole('region', { name: 'Table rows', exact: true })).toHaveCount(0);
    await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
    const screenshot = info.outputPath(`${driver}-unreadable-view.png`);
    await page.screenshot({ path: screenshot, fullPage: true });
    await info.attach(`${driver}-unreadable-view`, { path: screenshot, contentType: 'image/png' });
    await page.getByRole('button', { name: 'healthy', exact: true }).click();
    await expect(page.getByRole('region', { name: 'Table rows', exact: true })).toContainText('available');
    await expect(page.getByRole('alert')).toHaveCount(0);
    await page.getByRole('button', { name: 'a_broken view', exact: true }).click();
    await expect(page.getByRole('alert')).toContainText('Could not load a_broken.');
    await expect(page.getByRole('region', { name: 'Table rows', exact: true })).toHaveCount(0);
    await page.getByRole('tab', { name: 'SQL query', exact: true }).click();
    await page.locator('#db-sql').fill('DROP VIEW a_broken;');
    await page.getByRole('button', { name: 'Run selection / all', exact: true }).click();
    await expect(page.getByRole('alert')).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'a_broken view', exact: true })).toHaveCount(0);
    await page.getByRole('tab', { name: 'Rows', exact: true }).click();
    await expect(page.getByRole('region', { name: 'Table rows', exact: true })).toContainText('available');
    expect(bridge.errors()).toBe('');
  } finally { await bridge.close(); }
});

test('SQLite edits the original project file through the native worker with MySQL stopped', async ({ page }, info) => {
  test.skip(!process.env.FAIRWAY_TEST_RUNTIME, 'Set FAIRWAY_TEST_RUNTIME and build terminal_bridge.');
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
    await page.getByRole('button', { name: 'Back up SQLite file', exact: true }).click();
    await expect(page.getByRole('status')).toContainText('SQLite backup saved:');
    await unlink(selectedFile);
    await page.getByRole('button', { name: 'Refresh rows', exact: true }).click();
    await expect(page.getByRole('alert')).toContainText('missing');
    await expect(page.getByLabel('SQLite file path', { exact: true })).toHaveText(selectedFile);
    await page.getByRole('tab', { name: 'MySQL', exact: true }).click();
    const sqliteTab = page.getByRole('tab', { name: 'SQLite', exact: true });
    await sqliteTab.click();
    await expect(page.getByRole('alert')).toContainText('Open it again from its current location.');
    await expect(sqliteTab).toHaveAttribute('aria-selected', 'true');
    await expect(page.getByRole('button', { name: 'Open SQLite file', exact: true })).toBeEnabled();
    await expect(page.getByRole('button', { name: 'Reopen database.sqlite', exact: true })).toHaveCount(0);
    await page.reload();
    await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
    await sqliteTab.click();
    await expect(page.getByRole('heading', { name: 'Open a SQLite file to browse your data' })).toBeVisible();
    await expect(page.getByRole('alert')).toHaveCount(0);
    expect(bridge.errors()).toBe('');
  } finally { await bridge.close(); }
});
