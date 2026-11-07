import { expect, test } from '@playwright/test';
import type { Snapshot } from '../../src/types';

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  const initial = await page.evaluate(async () => {
    const path = '/src/ui-test/backend.ts';
    return (await import(path)).invoke('snapshot') as Promise<Snapshot>;
  });
  await page.addInitScript((snapshot: Snapshot) => {
    const callbacks = new Map<number, (event: { payload: unknown }) => void>(); let next = 0;
    const base = { type: 'text', nullable: true, defaultValue: null, extra: '', columnKey: '', expression: '', binary: false, generated: false };
    const columns = [{ ...base, name: 'id', type: 'bigint', nullable: false, columnKey: 'PRI' }, { ...base, name: 'body', type: 'longtext' }, { ...base, name: 'note' }, { ...base, name: 'constructor' }, { ...base, name: '__proto__' }];
    let body = '🟢é'.repeat(150000) + 'END'; let note: string | null = null;
    let version = 'a'.repeat(64), stale = false, oversized = false, hold = false;
    const held = new Map<string, (error: Error) => void>();
    window.addEventListener('test-stale', () => { stale = true; });
    window.addEventListener('test-oversize', () => { oversized = true; });
    window.addEventListener('test-hold', () => { hold = true; });
    Object.defineProperty(navigator, 'clipboard', { configurable: true, value: { async writeText(text: string) {
      document.documentElement.dataset.clipboard = JSON.stringify({ length: text.length, end: text.slice(-12), json: text.startsWith('{') ? { note: JSON.parse(text).note, bodyLength: JSON.parse(text).body.length } : null });
    } } });
    window.__TAURI_INTERNALS__ = {
      transformCallback(callback) { callbacks.set(++next, callback); return next; }, unregisterCallback(id) { callbacks.delete(id); },
      async invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
        if (command === 'plugin:event|listen') return ++next as T;
        if (command === 'plugin:event|unlisten') return undefined as T;
        if (command === 'startup_status') return { blocking: false, errors: [], dataDir: snapshot.dataDir } as T;
        if (command === 'startup_services') return { nginx: false, mysql: false, mail: false } as T;
        if (command === 'login_item_status') return { registered: false, state: 'notRegistered' } as T;
        if (command === 'snapshot') return structuredClone(snapshot) as T;
        if (command === 'cancel_data_browser') { document.documentElement.dataset.cancelled = 'yes'; held.get(String(args.id))?.(new Error('Canceled')); return undefined as T; }
        if (command !== 'data_browser') throw new Error(`Unexpected command ${command}`);
        const r = args.request as { op: string; column: string; offset: number; values: Record<string, string | null>; rowRef?: unknown; original?: unknown };
        if (r.op === 'databases') return { databases: ['school'] } as T;
        if (r.op === 'tables') return { tables: [{ name: 'large_values', kind: 'BASE TABLE' }] } as T;
        if (r.op === 'structure') return { columns, key: ['id'], kind: 'BASE TABLE', indexes: [], foreignKeys: [], referencedBy: [], sql: 'CREATE TABLE large_values' } as T;
        if (r.op === 'rows') return { columns, key: ['id'], kind: 'BASE TABLE', rows: [{ id: '9007199254740993', body: Array.from(body).slice(0, 1024).join(''), note, constructor: 'constructor field', ['__proto__']: 'prototype field' }], rowRefs: [{ key: { id: '9007199254740993' }, version }], truncated: [{ body: { length: Array.from(body).length } }], offset: 0, total: '1', limit: 50 } as T;
        if (r.op === 'cell') {
          document.documentElement.dataset.reads = String(Number(document.documentElement.dataset.reads ?? 0) + 1);
          if (hold) await new Promise<void>((_, reject) => held.set(String(args.id), reject));
          if (stale) throw new Error('This row changed or was deleted. Refresh before reading its value.');
          const chars = Array.from(body), end = Math.min(chars.length, r.offset + 131072);
          return { value: chars.slice(r.offset, end).join(''), offset: r.offset, nextOffset: end, total: oversized ? 20 * 1024 * 1024 : chars.length, complete: !oversized && end === chars.length } as T;
        }
        if (r.op === 'update') {
          if (stale) throw new Error('This row changed. Refresh before saving.');
          document.documentElement.dataset.saved = JSON.stringify({ values: Object.fromEntries(Object.entries(r.values).map(([name, value]) => [name, value === null ? null : value.length < 100 ? value : { length: value.length }])), hasOriginal: r.original !== undefined, rowRef: r.rowRef });
          if ('body' in r.values) body = r.values.body!;
          if ('note' in r.values) note = r.values.note;
          version = 'b'.repeat(64); return { message: 'Row saved.' } as T;
        }
        throw new Error(`Unexpected operation ${r.op}`);
      },
    };
  }, initial);
  await page.goto('http://127.0.0.1:1422/');
  await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Edit row 1', exact: true })).toBeEnabled();
});

test('saving another field keeps the complete large value without loading or sending its preview', async ({ page }) => {
  await expect(page.locator('.db-cell').filter({ hasText: '🟢' })).toHaveAttribute('title', /Preview.*300,003 characters/);
  await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Edit row', exact: true });
  await expect(dialog.getByLabel('body value mode', { exact: true })).toHaveValue('unchanged');
  await dialog.getByLabel('note value', { exact: true }).fill('small change');
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  const saved = JSON.parse((await page.locator('html').getAttribute('data-saved'))!);
  expect(saved.values).toEqual({ note: 'small change' }); expect(saved.hasOriginal).toBe(false);
  expect(saved.rowRef.key).toEqual({ id: '9007199254740993' });
  expect(await page.locator('html').getAttribute('data-reads')).toBeNull();
});

test('large values support bounded parts and complete field/JSON copies without copying a preview', async ({ page }, info) => {
  await page.getByRole('button', { name: 'View row 1', exact: true }).click();
  const view = page.getByRole('dialog', { name: 'View row', exact: true });
  await view.getByRole('button', { name: 'Read body in parts', exact: true }).click();
  await expect(view).toContainText('Characters 1–131072 of 300,003');
  await view.getByRole('button', { name: 'Next part', exact: true }).click();
  await expect(view).toContainText('Characters 131073–262144 of 300,003');
  await view.getByRole('button', { name: 'Previous part', exact: true }).click();
  await expect(view).toContainText('Characters 1–131072 of 300,003');
  await view.getByRole('button', { name: 'Copy body', exact: true }).click();
  await expect(view.getByRole('status')).toContainText('body copied');
  let copied = JSON.parse((await page.locator('html').getAttribute('data-clipboard'))!);
  expect(copied.length).toBe(450003); expect(copied.end).toMatch(/END$/);
  await view.getByRole('button', { name: 'Copy row JSON', exact: true }).click();
  await expect(view.getByRole('status')).toContainText('Row JSON copied');
  copied = JSON.parse((await page.locator('html').getAttribute('data-clipboard'))!);
  expect(copied.json).toEqual({ note: null, bodyLength: 450003 });
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  const path = info.outputPath('large-value-viewer.png'); await page.screenshot({ path }); await info.attach('large-value-viewer', { path, contentType: 'image/png' });
});

test('loading a field preserves other edits, and only an explicit replacement is submitted', async ({ page }) => {
  await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Edit row', exact: true });
  await dialog.getByLabel('note value', { exact: true }).fill('keep this edit');
  await dialog.getByRole('button', { name: 'Load body to edit', exact: true }).click();
  await expect(dialog.getByLabel('body value', { exact: true })).toHaveValue(/END$/);
  await expect(dialog.getByLabel('note value', { exact: true })).toHaveValue('keep this edit');
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  expect(JSON.parse((await page.locator('html').getAttribute('data-saved'))!).values).toEqual({ note: 'keep this edit' });
  await page.getByRole('button', { name: 'Edit row 1', exact: true }).click();
  await dialog.getByLabel('body value mode', { exact: true }).selectOption('value');
  await dialog.getByLabel('body value', { exact: true }).fill('replacement');
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  expect(JSON.parse((await page.locator('html').getAttribute('data-saved'))!).values).toEqual({ body: 'replacement' });
});

test('stale and oversized reads never reach the clipboard, and closing cancels an in-flight read', async ({ page }) => {
  await page.getByRole('button', { name: 'View row 1', exact: true }).click();
  const view = page.getByRole('dialog', { name: 'View row', exact: true });
  await page.evaluate(() => window.dispatchEvent(new Event('test-oversize')));
  await view.getByRole('button', { name: 'Copy body', exact: true }).click();
  await expect(view.getByRole('alert')).toContainText('16 MiB');
  expect(await page.locator('html').getAttribute('data-clipboard')).toBeNull();
  await page.evaluate(() => window.dispatchEvent(new Event('test-stale')));
  await view.getByRole('button', { name: 'Load full body', exact: true }).click();
  await expect(view.getByRole('alert')).toContainText('row changed');
  expect(await page.locator('html').getAttribute('data-clipboard')).toBeNull();
  await page.evaluate(() => window.dispatchEvent(new Event('test-hold')));
  await view.getByRole('button', { name: 'Load full body', exact: true }).click();
  await expect(view.getByRole('status')).toContainText('Loading');
  await view.getByRole('button', { name: 'Close', exact: true }).click();
  await expect(view).toHaveCount(0);
  await expect(page.locator('html')).toHaveAttribute('data-cancelled', 'yes');
});


test('valid column names that match JavaScript object properties remain ordinary fields', async ({ page }) => {
  await page.getByRole('button', { name: 'View row 1', exact: true }).click();
  const view = page.getByRole('dialog', { name: 'View row', exact: true });
  await expect(view.getByText('constructor field', { exact: true })).toBeVisible();
  await expect(view.getByText('prototype field', { exact: true })).toBeVisible();
  await expect(view.getByRole('button', { name: 'Load full constructor', exact: true })).toHaveCount(0);
  await view.getByRole('button', { name: 'Copy constructor', exact: true }).click();
  await expect(view.getByRole('status')).toContainText('constructor copied');
  expect(JSON.parse((await page.locator('html').getAttribute('data-clipboard'))!).length).toBe(17);
  await view.getByRole('button', { name: 'Edit', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Edit row', exact: true });
  await dialog.getByLabel('__proto__ value', { exact: true }).fill('edited prototype');
  await dialog.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(dialog).toHaveCount(0);
  expect(JSON.parse((await page.locator('html').getAttribute('data-saved'))!).values).toEqual(JSON.parse('{"__proto__":"edited prototype"}'));
});
