import { expect, test, type Page } from '@playwright/test';
async function configure(page: Page, options: Record<string, unknown>) { await page.evaluate(async value => { const path = '/src/ui-test/postgresql.ts'; (await import(path)).configure(value); }, options); }
async function inspect(page: Page) { return page.evaluate(async () => { const path = '/src/ui-test/postgresql.ts'; return (await import(path)).inspect(); }); }
async function open(page: Page) { await page.goto('/'); await page.getByLabel('Example paths').selectOption('macOS'); await page.getByRole('button', { name: 'PostgreSQL', exact: true }).click(); await expect(page.getByRole('button', { name: 'New connection', exact: true })).toBeEnabled(); }
async function connect(page: Page, external = false) {
  await page.getByRole('button', { name: 'New connection', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Connect to PostgreSQL', exact: true });
  if (external) { await dialog.getByLabel('Connection source').selectOption('external'); await dialog.getByLabel('PostgreSQL user').fill('fixture'); await dialog.getByLabel('PostgreSQL password').fill('private-form-marker'); }
  await dialog.getByLabel('Authorize this database connection').check(); await dialog.getByRole('button', { name: 'Connect', exact: true }).click();
  await expect(page.getByLabel('PostgreSQL schema')).toBeEnabled();
  await expect(page.getByText('Authenticated by last successful operation')).toBeVisible();
}
async function rows(page: Page) { await page.getByLabel('PostgreSQL schema', { exact: true }).selectOption('public'); await page.getByRole('button', { name: /students_日本/ }).click(); await expect(page.getByRole('region', { name: 'Rows table', exact: true })).toBeVisible(); }

test('status separates process from authentication and managed actions require explicit consent', async ({ page }) => {
  await open(page);
  const status = page.getByRole('region', { name: 'Managed PostgreSQL status' });
  await expect(status).toContainText('Running'); await expect(status).toContainText('Not checked');
  expect((await inspect(page)).calls.some((c: { command: string }) => c.command === 'postgres_managed_health')).toBe(false);
  await status.getByRole('button', { name: 'Check access', exact: true }).click(); await expect(status).toContainText('Verified by health check');
  await status.getByRole('button', { name: 'Stop', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Stop managed PostgreSQL', exact: true });
  await expect(dialog.getByRole('button', { name: 'Stop managed PostgreSQL', exact: true })).toBeDisabled();
  await dialog.getByLabel('Authorize this managed action').check(); await dialog.getByRole('button', { name: 'Stop managed PostgreSQL', exact: true }).click();
  await expect(status).toContainText('Stopped');
});
test('unsupported platforms explain availability without dispatching PostgreSQL requests', async ({ page }) => {
  await page.goto('/'); await page.getByLabel('Example paths').selectOption('Windows');
  await page.getByRole('button', { name: 'PostgreSQL', exact: true }).click();
  await expect(page.getByText('PostgreSQL browsing and managed services currently require an Apple Silicon Mac.', { exact: false })).toBeVisible();
  await expect(page.getByRole('button', { name: 'New connection', exact: true })).toHaveCount(0);
  expect((await inspect(page)).calls).toHaveLength(0);
});
test('browse exact PostgreSQL values, page/sort/filter, inspect columns/indexes/views and maximize', async ({ page }) => {
  await open(page); await connect(page); await rows(page);
  await expect(page.getByRole('region', { name: 'Rows table', exact: true })).toContainText('9007199254740993');
  await page.getByRole('button', { name: 'View document, column 2, row 1', exact: true }).click();
  const value = page.getByRole('dialog', { name: 'PostgreSQL value', exact: true });
  await expect(value.getByLabel('Exact PostgreSQL value')).toHaveValue('{"big":9007199254740993,"html":"<script>bad</script>"}');
  await page.keyboard.press('Escape'); await expect(value).toHaveCount(0);
  await page.getByRole('button', { name: 'Next rows', exact: true }).click(); await expect(page.getByText('Offset 50 · up to 50 rows')).toBeVisible();
  await page.getByLabel('PostgreSQL sort column').selectOption('id'); await page.getByLabel('PostgreSQL sort direction').selectOption('desc');
  await page.getByLabel('PostgreSQL filter column').selectOption('id'); await page.getByLabel('PostgreSQL filter operator').selectOption('gt'); await page.getByLabel('PostgreSQL filter value').fill('6');
  await page.getByRole('button', { name: 'Apply sort & filter' }).click(); await expect(page.getByText('Offset 0 · up to 50 rows')).toBeVisible(); await expect(page.getByRole('button', { name: 'Next rows', exact: true })).toBeDisabled();
  await page.getByRole('tab', { name: 'Columns', exact: true }).click(); await expect(page.getByRole('region', { name: 'Columns table', exact: true })).toContainText('jsonb');
  await page.getByRole('tab', { name: 'Indexes', exact: true }).click(); await expect(page.getByRole('region', { name: 'Indexes table', exact: true })).toContainText('students_pkey');
  await page.getByRole('button', { name: /student_view/ }).click(); await page.getByRole('tab', { name: 'View definition', exact: true }).click(); await expect(page.getByRole('region', { name: 'View definition table', exact: true })).toContainText('SELECT id');
  await page.getByRole('button', { name: 'Maximize PostgreSQL', exact: true }).click(); await expect(page.getByRole('dialog', { name: 'PostgreSQL maximized', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Restore PostgreSQL', exact: true }).click();
  const overflow = await page.evaluate(() => document.documentElement.scrollWidth > window.innerWidth); expect(overflow).toBe(false);
});
test('SQL defaults read-only, exact modifying statement needs one-time authorization and uncertainty is shown', async ({ page }) => {
  await open(page); await connect(page); await page.getByRole('tab', { name: 'SQL editor', exact: true }).click();
  await expect(page.getByLabel('SQL execution mode')).toHaveValue('readOnly');
  await page.getByRole('button', { name: 'Run SQL', exact: true }).click(); await expect(page.getByRole('region', { name: 'SQL result table', exact: true })).toContainText('9007199254740993'); await expect(page.getByText('Result truncated.', { exact: false })).toBeVisible();
  await page.getByLabel('PostgreSQL SQL', { exact: true }).fill('UPDATE fixture SET value=1'); await page.getByLabel('SQL execution mode').selectOption('readWrite'); await page.getByRole('button', { name: 'Run SQL', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Authorize modifying SQL', exact: true });
  await expect(dialog.getByRole('button', { name: 'Authorize and run once' })).toBeDisabled();
  expect((await inspect(page)).calls.filter((c: { mode?: string }) => c.mode === 'readWrite')).toHaveLength(0);
  await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await configure(page, { fail: 'query' }); await page.getByRole('button', { name: 'Run SQL', exact: true }).click();
  await page.getByLabel('I authorize this exact modifying statement on this database').check(); await page.getByRole('button', { name: 'Authorize and run once' }).click();
  await expect(page.getByRole('alert').filter({ hasText: 'Outcome unknown' })).toBeVisible(); await expect(page.getByLabel('SQL execution mode')).toHaveValue('readOnly');
  await page.getByRole('button', { name: 'Refresh status', exact: true }).click(); await expect(page.getByRole('alert').filter({ hasText: 'Outcome unknown' })).toBeVisible();
});
test('Cancel owns the current query; disconnect drops late data and external navigation re-prompts for password', async ({ page }) => {
  await open(page); await connect(page, true);
  await expect(page.getByText('External server · no lifecycle ownership')).toBeVisible();
  await page.getByLabel('PostgreSQL database').selectOption('practice 日本');
  const dialog = page.getByRole('dialog', { name: 'Connect to PostgreSQL', exact: true }); await expect(dialog.getByLabel('PostgreSQL password')).toHaveValue('');
  await expect(dialog).toContainText('Remote connections are unavailable'); await dialog.getByRole('button', { name: 'Cancel', exact: true }).click();
  await page.getByRole('tab', { name: 'SQL editor', exact: true }).click(); await configure(page, { delay: 'query' }); await page.getByRole('button', { name: 'Run SQL', exact: true }).click();
  await page.getByRole('button', { name: 'Cancel Query', exact: true }).click(); await expect(page.getByRole('alert').filter({ hasText: 'Operation cancelled' })).toBeVisible();
  await configure(page, { delay: 'query', ignoreCancel: true }); await page.getByRole('button', { name: 'Run SQL', exact: true }).click(); await page.getByRole('button', { name: 'Disconnect', exact: true }).click();
  await expect(page.getByText('No PostgreSQL connection', { exact: true })).toBeVisible(); await expect(page.getByRole('region', { name: 'SQL result table', exact: true })).toHaveCount(0);
  await expect.poll(async () => (await inspect(page)).sessions.length).toBe(0);
  const storage = await page.evaluate(() => JSON.stringify([localStorage, sessionStorage])); expect(storage).not.toContain('private-form-marker');
});
test('loading, empty schemas, unavailable credentials and recovery keep SQL tools accessible', async ({ page }) => {
  await open(page); await configure(page, { fail: 'connection' });
  await page.getByRole('button', { name: 'New connection', exact: true }).click(); await page.getByLabel('Authorize this database connection').check(); await page.getByRole('button', { name: 'Connect', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Fixture connection'); await expect(page.getByText('Access failed — retry or reconnect')).toBeVisible();
  await configure(page, { empty: true }); await page.getByRole('button', { name: 'Refresh explorer', exact: true }).click(); await expect(page.getByText('No schemas available.', { exact: false })).toBeVisible();
  await page.getByRole('tab', { name: 'SQL editor', exact: true }).click(); await configure(page, { fail: 'query' }); await page.getByRole('button', { name: 'Run SQL', exact: true }).click(); await expect(page.getByRole('alert')).toContainText('Operation timed out');
});
