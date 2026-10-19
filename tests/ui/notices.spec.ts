import { expect, test } from '@playwright/test';
import type { Locator, Page } from '@playwright/test';
import type { Snapshot } from '../../src/types';

// Use the production UI with a controlled desktop bridge, including native
// error events and delayed failures that the ordinary scenarios don't expose.
test.beforeEach(async ({ page }) => {
  await page.goto('/');
  const snapshot = await page.evaluate(async () => {
    const modulePath = '/src/ui-test/backend.ts';
    return (await import(modulePath)).invoke('snapshot') as Promise<Snapshot>;
  });
  await page.addInitScript((initial: Snapshot) => {
    const data = structuredClone(initial);
    const callbacks = new Map<number, (event: { payload: unknown }) => void>();
    const listeners = new Map<string, number>();
    let next = 0;
    let fail = false;
    window.addEventListener('test-fail-operation', () => { fail = true; });
    window.addEventListener('test-start-mysql', () => {
      Object.assign(data.services.find(service => service.id === 'mysql')!, { running: true, ready: true });
      const id = listeners.get('fairway-status');
      if (id) callbacks.get(id)?.({ payload: structuredClone(data) });
    });
    window.addEventListener('test-backend-error', event => {
      const id = listeners.get('fairway-error');
      if (id) callbacks.get(id)?.({ payload: (event as CustomEvent).detail });
    });
    window.__TAURI_INTERNALS__ = {
      transformCallback(callback) { callbacks.set(++next, callback); return next; },
      unregisterCallback(id) { callbacks.delete(id); },
      async invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
        if (command === 'plugin:event|listen') { listeners.set(String(args.event), Number(args.handler)); return next as T; }
        if (command === 'plugin:event|unlisten') return undefined as T;
        if (command === 'snapshot') return structuredClone(data) as T;
        await new Promise(resolve => setTimeout(resolve, 600));
        if (fail) { fail = false; throw new Error('Could not complete the operation. Try again.'); }
        if (command === 'backup_database') return '/Users/Student May/Backups/classroom.sql' as T;
        if (command === 'cli_integration') { data.pathInstalled = Boolean(args.enabled); return structuredClone(data) as T; }
        if (command === 'set_project_https' && !args.enabled) {
          data.projects.find(project => project.domain === args.domain)!.https = false;
          throw new Error(`HTTPS is off for ${args.domain}.test, but certificate cleanup did not finish. Choose Remove certificate trust to retry. Certificate removal was cancelled.`);
        }
        if (command === 'remove_https_trust') { data.httpsTrusted = false; return structuredClone(data) as T; }
        throw new Error(`Unexpected test command: ${command}`);
      },
    };
  }, snapshot);
  await page.goto('http://127.0.0.1:1422/');
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeEnabled();
});

async function positionAt(page: Page, control: Locator) {
  await control.scrollIntoViewIfNeeded();
  await control.focus();
  const position = await page.evaluate(() => window.scrollY);
  expect(position).toBeGreaterThan(100);
  return position;
}

test('cancelled last-site certificate cleanup leaves HTTPS off and can be retried', async ({ page }) => {
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
  const shop = page.getByRole('switch', { name: 'HTTPS for My Shop' });
  await expect(shop).toBeChecked();
  await shop.locator('..').click();
  await expect(shop).not.toBeChecked();
  await expect(page.getByRole('alert')).toContainText('HTTPS is off for shop.test, but certificate cleanup did not finish');
  await page.getByRole('button', { name: 'Remove certificate trust', exact: true }).click();
  await expect(page.getByRole('status')).toContainText('local certificate is no longer trusted');
  await expect(page.getByRole('button', { name: 'Remove certificate trust', exact: true })).toHaveCount(0);
  await expect(shop).not.toBeChecked();
});

async function expectPosition(page: Page, control: Locator, position: number) {
  await expect(control).toBeEnabled();
  await expect(control).toBeFocused();
  // Wait across frames so a delayed smooth scroll or focus restoration cannot
  // pass by reporting only the first frame after completion.
  const samples = await page.evaluate(async () => {
    const positions: number[] = [];
    for (let i = 0; i < 45; i++) {
      await new Promise(requestAnimationFrame);
      positions.push(window.scrollY);
    }
    return positions;
  });
  expect(samples.every(value => Math.abs(value - position) <= 1)).toBe(true);
}

test('CLI enable and disable preserve the viewport and keyboard focus', async ({ page }, info) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const tools = page.getByRole('region', { name: 'Command-line tools', exact: true });
  const toggle = tools.getByRole('button', { name: /Remove from user PATH|Enable command-line tools/ });
  for (const enabled of [false, true]) {
    const position = await positionAt(page, toggle);
    await toggle.press('Enter');
    const result = page.getByRole('status').filter({ hasText: enabled ? 'added to your user PATH' : 'removed from your user PATH' });
    await expect(result).toContainText('Open a new terminal or restart VS Code.');
    await expect(result).toBeInViewport({ ratio: 1 });
    await expectPosition(page, toggle, position);
  }
  await info.attach('cli-result', { body: await page.screenshot(), contentType: 'image/png' });
});

test('lower-page failures stay visible without moving focus or the viewport', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const toggle = page.getByRole('button', { name: 'Remove from user PATH', exact: true });
  const position = await positionAt(page, toggle);
  await page.evaluate(() => window.dispatchEvent(new Event('test-fail-operation')));
  await toggle.press('Enter');
  await expect(page.getByRole('alert')).toHaveText('Could not complete the operation. Try again.');
  await expect(page.getByRole('alert')).toBeInViewport({ ratio: 1 });
  await expectPosition(page, toggle, position);
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('alert')).toHaveCount(0);
});

test('navigation clears results and suppresses late operation messages', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const toggle = page.getByRole('button', { name: 'Remove from user PATH', exact: true });
  await toggle.click();
  await expect(toggle).toBeDisabled();
  const environment = page.getByRole('button', { name: 'Environment', exact: true });
  await environment.focus();
  await environment.press('Enter');
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeEnabled();
  await expect(environment).toBeFocused();
  await expect(page.locator('.operation-notice')).toHaveText('');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.locator('.operation-notice')).toHaveText('');
  await expect(page.getByRole('button', { name: 'Enable command-line tools' })).toBeEnabled();
});

test('backend errors are persistent, bounded, and do not repeat after dismissal', async ({ page }) => {
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const control = page.getByRole('button', { name: 'Open backups', exact: true });
  const position = await positionAt(page, control);
  const error = `Backend unavailable. ${'Long actionable error detail. '.repeat(150)}`;
  const emit = async (text: string) => page.evaluate(payload => window.dispatchEvent(new CustomEvent('test-backend-error', { detail: payload })), text);
  await emit(error);
  const alert = page.getByRole('alert');
  await expect(alert).toHaveText(error.trim());
  await expect(page.getByRole('button', { name: 'Dismiss message' })).toBeInViewport({ ratio: 1 });
  expect(await page.locator('.operation-notice').evaluate(element => element.getBoundingClientRect().height)).toBeLessThan(270);
  await expectPosition(page, control, position);
  await page.getByRole('button', { name: 'Dismiss message' }).click();
  await emit(error);
  await expect(alert).toHaveCount(0);
  await emit('A different backend error.');
  await expect(alert).toHaveText('A different backend error.');
});


test('database backup success and failure preserve the lower-page viewport', async ({ page }) => {
  await page.evaluate(() => window.dispatchEvent(new Event('test-start-mysql')));
  const backup = page.getByRole('button', { name: 'Back up databases', exact: true });
  await expect(backup).toBeEnabled();
  const position = await positionAt(page, backup);
  await backup.press('Enter');
  const status = page.getByRole('status').filter({ hasText: 'Database backup saved:' });
  await expect(status).toBeInViewport({ ratio: 1 });
  await expectPosition(page, backup, position);
  await page.evaluate(() => window.dispatchEvent(new Event('test-fail-operation')));
  await backup.press('Enter');
  await expect(page.getByRole('alert')).toHaveText('Could not complete the operation. Try again.');
  await expect(page.getByRole('alert')).toBeInViewport({ ratio: 1 });
  await expectPosition(page, backup, position);
});
