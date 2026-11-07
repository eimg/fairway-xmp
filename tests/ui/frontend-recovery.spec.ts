import { expect, test } from '@playwright/test';
import type { Snapshot } from '../../src/types';

// Exercise the production components with controlled native responses and races.
test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  const initial = await page.evaluate(async () => {
    const path = '/src/ui-test/backend.ts';
    return (await import(path)).invoke('snapshot') as Promise<Snapshot>;
  });
  await page.addInitScript((snapshot: Snapshot) => {
    const data = structuredClone(snapshot);
    const callbacks = new Map<number, (event: { payload: unknown }) => void>();
    const listeners = new Map<string, number>();
    const output = new Map<string, string>();
    let next = 0, closes = 0;
    let badRow = false;
    let gate: Promise<void> | null = null;
    let release = () => {};
    const requests: string[] = [], writes: string[] = [];
    function emit(event: string, payload: unknown) { const id = listeners.get(event); if (id) callbacks.get(id)?.({ payload }); }
    window.addEventListener('test-service', event => {
      Object.assign(data.services.find(service => service.id === 'mysql')!, (event as CustomEvent).detail);
      emit('fairway-status', structuredClone(data));
    });
    window.addEventListener('test-bad-row', event => { badRow = (event as CustomEvent).detail; });
    window.addEventListener('test-hold-sqlite', () => { gate = new Promise(resolve => { release = resolve; }); });
    window.addEventListener('test-release-sqlite', () => { release(); gate = null; });
    const column = { name: 'id', type: 'int', nullable: false, defaultValue: null, extra: '', columnKey: 'PRI', expression: '', binary: false, generated: false };
    window.__TAURI_INTERNALS__ = {
      transformCallback(callback) { callbacks.set(++next, callback); return next; }, unregisterCallback(id) { callbacks.delete(id); },
      async invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
        if (command === 'plugin:event|listen') { listeners.set(String(args.event), Number(args.handler)); return next as T; }
        if (command === 'plugin:event|unlisten') return undefined as T;
        if (command === 'startup_status') return { blocking: false, errors: [], dataDir: data.dataDir } as T;
        if (command === 'startup_services') return { nginx: false, mysql: false, mail: false } as T;
        if (command === 'login_item_status') return { registered: false, state: 'notRegistered' } as T;
        if (command === 'snapshot') return structuredClone(data) as T;
        if (command === 'read_logs') return { entries: [{ id: 'old-file:1', source: 'php', file: 'php.log', severity: 'error', time: null, text: 'Old file failure' }], unavailable: [], limited: false } as T;
        if (command === 'terminal_context') return { defaultDirectory: '/Users/Student/www', folders: [], sites: [], historySupported: false } as T;
        if (command === 'terminal_start') { const request = args.request as { id: string }; output.set(request.id, 'retained terminal session\r\n'); return { shell: 'zsh', directory: '/Users/Student/www' } as T; }
        if (command === 'terminal_read') { const text = output.get(String(args.id)) ?? ''; output.set(String(args.id), ''); return { data: [...new TextEncoder().encode(text)], exited: false, exitCode: null, error: null } as T; }
        if (command === 'terminal_write') { writes.push(String(args.data)); document.documentElement.dataset.writes = JSON.stringify(writes); return undefined as T; }
        if (command === 'terminal_close') { document.documentElement.dataset.closes = String(++closes); return undefined as T; }
        if (command === 'terminal_resize') return undefined as T;
        if (command === 'pick_sqlite_file') return '/Users/Student/classroom.sqlite' as T;
        if (command === 'cancel_data_browser') return undefined as T;
        if (command === 'data_browser') {
          const request = args.request as { driver: string; path: string; op: string };
          requests.push(`${request.driver}:${request.op}`); document.documentElement.dataset.requests = JSON.stringify(requests);
          if (request.driver === 'sqlite' && gate) await gate;
          const label = request.driver === 'sqlite' ? 'sqlite_kept' : 'mysql_rows';
          if (request.op === 'databases') return { databases: [request.driver === 'sqlite' ? request.path : 'mysql_school'] } as T;
          if (request.op === 'tables') return { tables: [{ name: label, kind: 'BASE TABLE' }] } as T;
          if (request.op === 'structure') return { columns: [column], key: ['id'], kind: 'BASE TABLE', indexes: [], foreignKeys: [], referencedBy: [], sql: 'CREATE TABLE example (id INT)' } as T;
          if (request.op === 'rows') return { columns: [column], key: ['id'], kind: 'BASE TABLE', rows: [{ id: badRow ? { invalid: true } : '42' }], total: '1', offset: 0, limit: 50 } as T;
        }
        throw new Error(`Unexpected test command: ${command}`);
      },
    };
  }, initial);
  await page.goto('http://127.0.0.1:1422/');
  await expect(page.getByRole('button', { name: 'Stop all', exact: true })).toBeVisible();
});

test('clear retains paused history and reveals identical new service occurrences after navigation and hiding', async ({ page }) => {
  const publish = (generation: number) => page.evaluate(messageGeneration => window.dispatchEvent(new CustomEvent('test-service', { detail: { message: 'Repeated connection failure', messageGeneration } })), generation);
  async function action(name: string) { await page.getByRole('button', { name: 'Log options', exact: true }).click(); await page.getByRole('menuitem', { name, exact: true }).click(); }
  await publish(1);
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  const entries = page.getByRole('region', { name: 'Log entries', exact: true });
  await expect(entries).toContainText('Repeated connection failure');
  await action('Pause'); await action('Clear view');
  await expect(entries).toContainText('Log view cleared');
  await publish(2); // There is no observable null/error transition between retries.
  await expect(entries).toContainText('Log view cleared');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: true }); document.dispatchEvent(new Event('visibilitychange')); });
  await publish(3);
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await expect(entries).toContainText('Log view cleared');
  await page.evaluate(() => { Object.defineProperty(document, 'hidden', { configurable: true, value: false }); document.dispatchEvent(new Event('visibilitychange')); });
  await action('Resume');
  await expect(entries).toContainText('Repeated connection failure');
  await expect(entries).not.toContainText('Old file failure');
});

test('plain control keys reach the shell and platform find/zoom shortcuts stay available', async ({ page }) => {
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
  await expect(page.locator('.xterm-rows')).toContainText('retained terminal session');
  await input.press('Control+f');
  await expect.poll(() => page.locator('html').getAttribute('data-writes')).toContain('\\u0006');
  await expect(page.getByRole('textbox', { name: 'Find in terminal', exact: true })).toHaveCount(0);
  const macos = await page.evaluate(() => navigator.platform.startsWith('Mac'));
  await input.press(macos ? 'Meta+f' : 'Control+Shift+f');
  await expect(page.getByRole('textbox', { name: 'Find in terminal', exact: true })).toBeFocused();
});

test('a ready transition during SQLite opening cannot publish a stale MySQL catalog', async ({ page }) => {
  await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
  await expect(page.getByRole('button', { name: 'mysql_rows', exact: true })).toBeVisible();
  await page.evaluate(() => window.dispatchEvent(new Event('test-hold-sqlite')));
  await page.getByRole('button', { name: 'Open SQLite file…', exact: true }).click();
  await expect.poll(() => page.locator('html').getAttribute('data-requests')).toContain('sqlite:databases');
  for (const ready of [false, true]) {
    await page.evaluate(ready => window.dispatchEvent(new CustomEvent('test-service', { detail: { ready } })), ready);
    await page.waitForTimeout(50);
  }
  await page.evaluate(() => window.dispatchEvent(new Event('test-release-sqlite')));
  await expect(page.getByRole('tab', { name: 'SQLite', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByRole('button', { name: 'sqlite_kept', exact: true })).toBeVisible();
  await expect(page.getByRole('button', { name: 'mysql_rows', exact: true })).toHaveCount(0);
  const requests: string[] = JSON.parse((await page.locator('html').getAttribute('data-requests'))!);
  const start = requests.indexOf('sqlite:databases');
  expect(requests.slice(start).every(request => request.startsWith('sqlite:'))).toBe(true);
});

test('a failed maximized data view can retry without reloading or replacing its sibling terminal', async ({ page }, info) => {
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
  await expect(page.locator('.xterm-rows')).toContainText('retained terminal session');
  const original = await input.elementHandle();
  await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
  await expect(page.getByRole('button', { name: 'mysql_rows', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Maximize Data browser', exact: true }).click();
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('test-bad-row', { detail: true })));
  await page.getByRole('button', { name: 'Refresh rows', exact: true }).click();
  const recovery = page.getByRole('region', { name: 'Data browser recovery', exact: true });
  await expect(recovery).toBeVisible();
  const screenshot = info.outputPath('workspace-recovery.png');
  await page.screenshot({ path: screenshot });
  await info.attach('workspace-recovery', { path: screenshot, contentType: 'image/png' });
  await expect(page.locator('.sidebar')).not.toHaveAttribute('inert');
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('test-bad-row', { detail: false })));
  await recovery.getByRole('button', { name: 'Retry view', exact: true }).click();
  await expect(page.getByRole('button', { name: 'mysql_rows', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  expect(await input.evaluate((element, previous) => element === previous, original)).toBe(true);
  await expect(page.locator('.xterm-rows')).toContainText('retained terminal session');
  expect(await page.locator('html').getAttribute('data-closes')).toBeNull();
});

test('a failed lazy terminal chunk offers an explicit reload and leaves navigation usable', async ({ page }) => {
  await page.route(/\/assets\/TerminalPage-[^/]+\.js$/, route => route.fulfill({ status: 503, body: 'Temporarily unavailable', contentType: 'text/javascript', headers: { 'cache-control': 'no-store' } }));
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  const recovery = page.getByRole('region', { name: 'Terminal recovery', exact: true });
  await expect(recovery).toBeVisible();
  await expect(recovery).toContainText('Reloading closes embedded terminal sessions');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop all', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(recovery).toBeVisible();
  await page.unroute(/\/assets\/TerminalPage-[^/]+\.js$/);
  await Promise.all([page.waitForEvent('load'), recovery.getByRole('button', { name: 'Reload interface', exact: true }).click()]);
  await expect(page.getByRole('button', { name: 'Stop all', exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(page.locator('.xterm-rows')).toContainText('retained terminal session');
});
