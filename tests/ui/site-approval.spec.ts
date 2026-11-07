import { expect, test, type Page } from '@playwright/test';
import type { ProjectConfig, Snapshot } from '../../src/types';

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  const initial = await page.evaluate(async () => {
    const path = '/src/ui-test/backend.ts';
    return (await import(path)).invoke('snapshot') as Promise<Snapshot>;
  });
  await page.addInitScript((snapshot: Snapshot) => {
    const data = structuredClone(snapshot);
    data.os = 'macos'; data.projects = []; data.domainsPending = false; data.domainsApprovalPending = false; data.httpsTrusted = false;
    const callbacks = new Map<number, (event: { payload: unknown }) => void>();
    const listeners = new Map<number, { event: string; handler: number }>();
    let next = 0, saves = 0, settings = 0, retries = 0, approved = false;
    const emit = (event: string, payload: unknown) => {
      for (const listener of listeners.values()) if (listener.event === event) callbacks.get(listener.handler)?.({ payload });
    };
    const finish = () => {
      for (const site of data.projects) site.domainReady = true;
      data.domainsPending = false; data.domainsApprovalPending = false;
      emit('fairway-status', structuredClone(data));
    };
    window.addEventListener('test-helper-approved', () => { approved = true; finish(); });
    window.addEventListener('test-helper-allowed-without-notification', () => { approved = true; });
    window.addEventListener('test-address-write-failed', () => {
      approved = true; data.domainsApprovalPending = false;
      emit('fairway-status', structuredClone(data));
      emit('fairway-action-error', 'Address setup could not finish. Choose Set up addresses to retry. XPC write failed.');
    });
    window.__TAURI_INTERNALS__ = {
      transformCallback(callback) { callbacks.set(++next, callback); return next; },
      unregisterCallback(id) { callbacks.delete(id); },
      async invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
        if (command === 'plugin:event|listen') { listeners.set(++next, { event: args.event as string, handler: args.handler as number }); return next as T; }
        if (command === 'plugin:event|unlisten') { listeners.delete(args.eventId as number); return undefined as T; }
        if (command === 'startup_status') return { blocking: false, errors: [], dataDir: data.dataDir } as T;
        if (command === 'startup_services') return { nginx: false, mysql: false, mail: false } as T;
        if (command === 'login_item_status') return { registered: false, state: 'notRegistered' } as T;
        if (command === 'snapshot') return structuredClone(data) as T;
        if (command === 'pick_folder') return '/Users/fixture/Code/shop/public' as T;
        if (command === 'save_project') {
          saves++; settings++;
          const project = args.project as ProjectConfig;
          const saved = { ...project, available: true, domainReady: false };
          data.projects = args.previous ? data.projects.map(site => site.domain === args.previous ? saved : site) : [...data.projects, saved];
          data.domainsPending = true; data.domainsApprovalPending = true;
          return structuredClone(data) as T;
        }
        if (command === 'sync_domains') {
          retries++;
          if (approved) finish(); else if (!args.checkApproval) settings++;
          return structuredClone(data) as T;
        }
        if (command === 'remove_project') { data.projects = data.projects.filter(site => site.domain !== args.domain); return structuredClone(data) as T; }
        if (command === 'test-counts') return { saves, settings, retries } as T;
        throw new Error(`Unexpected test command: ${command}`);
      },
    };
  }, initial);
  await page.goto('http://127.0.0.1:1422/');
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
});

async function addSite(page: Page) {
  await page.getByRole('button', { name: 'Add site', exact: true }).click();
  const form = page.getByRole('dialog', { name: 'Add site', exact: true });
  await form.getByRole('button', { name: 'Choose…', exact: true }).click();
  await form.getByRole('textbox', { name: 'Site name', exact: true }).fill('Shop');
  await form.getByRole('button', { name: 'Add site', exact: true }).click();
  await expect(form).toHaveCount(0);
  await expect(page.getByRole('listitem', { name: 'Shop', exact: true })).toContainText('Waiting for macOS approval');
  await expect(page.getByRole('button', { name: 'Open Shop', exact: true })).toBeDisabled();
  await expect(page.getByRole('status').filter({ hasText: 'Once macOS enables the helper' })).toBeVisible();
  await expect(page.getByRole('alert')).toHaveCount(0);
}

test('adding a site waits honestly and completes after approval without another save or retry', async ({ page }) => {
  await addSite(page);
  await expect(page.getByRole('button', { name: 'Add site', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop all', exact: true })).toBeEnabled();
  await page.evaluate(() => window.dispatchEvent(new Event('test-helper-approved')));
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Open Shop', exact: true })).toBeEnabled();
  await expect(page.getByRole('listitem', { name: 'Shop', exact: true })).toHaveCount(1);
  await expect(page.getByText('Waiting for macOS approval', { exact: true })).toHaveCount(0);
  expect(await page.evaluate(() => window.__TAURI_INTERNALS__!.invoke('test-counts'))).toEqual({ saves: 1, settings: 1, retries: 0 });
});

test('leaving approval unfinished keeps the site pending and reopening Settings never claims success', async ({ page }) => {
  await addSite(page);
  await page.evaluate(() => { window.dispatchEvent(new Event('focus')); document.dispatchEvent(new Event('visibilitychange')); });
  await expect(page.getByRole('button', { name: 'Open Shop', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Open System Settings', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Open System Settings', exact: true })).toBeEnabled();
  await expect(page.getByRole('listitem', { name: 'Shop', exact: true })).toContainText('Waiting for macOS approval');
  await expect(page.getByText('Site addresses are set up.', { exact: true })).toHaveCount(0);
  await expect(page.getByRole('alert')).toHaveCount(0);
  expect(await page.evaluate(() => window.__TAURI_INTERNALS__!.invoke('test-counts'))).toEqual({ saves: 1, settings: 2, retries: 1 });
});

test('a failed post-approval write stays pending and offers an explicit successful retry', async ({ page }) => {
  await addSite(page);
  await page.evaluate(() => window.dispatchEvent(new Event('test-address-write-failed')));
  await expect(page.getByRole('alert')).toContainText('XPC write failed');
  await expect(page.getByRole('button', { name: 'Open Shop', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Set up addresses', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Open Shop', exact: true })).toBeEnabled();
  await expect(page.getByRole('status')).toContainText('Site addresses are set up.');
  expect(await page.evaluate(() => window.__TAURI_INTERNALS__!.invoke('test-counts'))).toEqual({ saves: 1, settings: 1, retries: 1 });
});

test('removing a site during approval does not recreate it when approval arrives', async ({ page }) => {
  await addSite(page);
  await page.getByRole('button', { name: 'Actions for Shop', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Remove site', exact: true }).click();
  await page.getByRole('dialog').getByRole('button', { name: 'Remove site', exact: true }).click();
  await expect(page.getByRole('listitem', { name: 'Shop', exact: true })).toHaveCount(0);
  await page.evaluate(() => window.dispatchEvent(new Event('test-helper-approved')));
  await expect(page.getByText('No sites yet.', { exact: true })).toBeVisible();
  await expect(page.getByRole('listitem', { name: 'Shop', exact: true })).toHaveCount(0);
});


test('Check approval completes a missed native notification without reopening Settings', async ({ page }) => {
  await addSite(page);
  await page.evaluate(() => window.dispatchEvent(new Event('test-helper-allowed-without-notification')));
  await expect(page.getByRole('button', { name: 'Open Shop', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Check approval', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Open Shop', exact: true })).toBeEnabled();
  await expect(page.getByRole('status')).toContainText('Site addresses are set up.');
  expect(await page.evaluate(() => window.__TAURI_INTERNALS__!.invoke('test-counts'))).toEqual({ saves: 1, settings: 1, retries: 1 });
});

test('Check approval shows a useful waiting result and removes it after later completion', async ({ page }) => {
  await addSite(page);
  await page.getByRole('button', { name: 'Check approval', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'macOS still reports' })).toContainText('turn it off and on again');
  await expect(page.getByRole('button', { name: 'Open Shop', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Check approval', exact: true })).toBeEnabled();
  await expect(page.getByRole('alert')).toHaveCount(0);
  expect(await page.evaluate(() => window.__TAURI_INTERNALS__!.invoke('test-counts'))).toEqual({ saves: 1, settings: 1, retries: 1 });
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth)).toBe(true);
  await page.evaluate(() => window.dispatchEvent(new Event('test-helper-approved')));
  await expect(page.getByRole('button', { name: 'Open Shop', exact: true })).toBeEnabled();
  await expect(page.getByText('macOS still reports', { exact: false })).toHaveCount(0);
});
