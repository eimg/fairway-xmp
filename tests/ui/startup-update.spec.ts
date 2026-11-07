import { expect, test } from '@playwright/test';
import type { Snapshot } from '../../src/types';

test('startup errors stay visible and retry uses the repaired saved configuration', async ({ page }) => {
  await page.goto('/');
  const initial = await page.evaluate(async () => {
    const path = '/src/ui-test/backend.ts';
    return (await import(path)).invoke('snapshot') as Promise<Snapshot>;
  });
  await page.addInitScript((snapshot: Snapshot) => {
    let repaired = false;
    let retryCount = 0;
    window.addEventListener('test-repair-settings', () => { repaired = true; snapshot.ports.website = 8899; snapshot.services.find(service => service.id === 'nginx')!.port = 8899; });
    const status = () => ({ blocking: !repaired, errors: repaired ? [] : ['The saved settings file is invalid. It has been preserved.'], dataDir: snapshot.dataDir });
    window.__TAURI_INTERNALS__ = {
      transformCallback() { return 1; }, unregisterCallback() {},
      async invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
        if (command.startsWith('plugin:event|')) return 1 as T;
        if (command === 'startup_status') return status() as T;
        if (command === 'retry_startup') { retryCount++; return status() as T; }
        if (command === 'snapshot') { if (!repaired) throw new Error('startup blocked'); return structuredClone(snapshot) as T; }
        if (command === 'startup_services') return { nginx: false, mysql: false, mail: false } as T;
        if (command === 'open_target' && args.target === 'data') return undefined as T;
        if (command === 'test-retry-count') return retryCount as T;
        throw new Error(`Unexpected command: ${command}`);
      },
    };
  }, initial);
  await page.goto('http://127.0.0.1:1422/');
  const recovery = page.getByRole('region', { name: 'Startup recovery' });
  await expect(recovery).toBeVisible();
  await expect(recovery.getByRole('alert')).toContainText('has been preserved');
  await expect(page.getByText('Loading Fairway XMP and starting selected services…')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toHaveCount(0);
  await recovery.getByRole('button', { name: 'Open data folder' }).click();
  await recovery.getByRole('button', { name: 'Retry startup' }).click();
  await expect(recovery.getByRole('button', { name: 'Retry startup' })).toBeEnabled();
  await expect(recovery.getByRole('alert')).toContainText('invalid');
  await page.evaluate(() => window.dispatchEvent(new Event('test-repair-settings')));
  await recovery.getByRole('button', { name: 'Retry startup' }).click();
  await expect(recovery).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeEnabled();
  await expect(page.locator('.services-grid')).toContainText(':8899');
  expect(await page.evaluate(() => window.__TAURI_INTERNALS__!.invoke('test-retry-count'))).toBe(2);
  expect(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
});

test('removed PHP selection can be replaced in Settings while service startup stays disabled', async ({ page }) => {
  await page.goto('/');
  const initial = await page.evaluate(async () => {
    const path = '/src/ui-test/backend.ts';
    return (await import(path)).invoke('snapshot') as Promise<Snapshot>;
  });
  await page.addInitScript((snapshot: Snapshot) => {
    snapshot.phpVersion = '8.1';
    snapshot.runtimeReady = false;
    const status = () => ({ blocking: false, errors: snapshot.phpVersion === '8.1' ? ['PHP 8.1 is no longer included. Open Settings to choose a bundled PHP version.'] : [], dataDir: snapshot.dataDir });
    window.__TAURI_INTERNALS__ = {
      transformCallback() { return 1; }, unregisterCallback() {},
      async invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
        if (command.startsWith('plugin:event|')) return 1 as T;
        if (command === 'startup_status' || command === 'retry_startup') return status() as T;
        if (command === 'snapshot') return structuredClone(snapshot) as T;
        if (command === 'startup_services') return { nginx: false, mysql: false, mail: false } as T;
        if (command === 'login_item_status') return { registered: false, state: 'notRegistered' } as T;
        if (command === 'get_php_settings') return { version: String(args.version), settings: { memoryLimitMb: 256, maxExecutionTime: 120, uploadMaxFilesizeMb: 64, timezone: 'UTC', displayErrors: true, sessionUploadProgress: true }, customized: false, timezones: ['UTC'] } as T;
        if (command === 'save_settings') {
          if (!snapshot.phpVersions[String(args.version)]) throw new Error('Unavailable PHP');
          snapshot.phpVersion = String(args.version);
          snapshot.runtimeReady = true;
          return structuredClone(snapshot) as T;
        }
        throw new Error(`Unexpected command: ${command}`);
      },
    };
  }, initial);
  await page.goto('http://127.0.0.1:1422/');
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeDisabled();
  await expect(page.getByRole('region', { name: 'Startup recovery' })).toContainText('PHP 8.1 is no longer included');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByText('Advanced settings', { exact: true }).click();
  const version = page.getByRole('combobox', { name: /^PHP version/ });
  await expect(version).toBeEnabled();
  await expect(version).toHaveValue('8.1');
  await expect(page.getByRole('button', { name: 'Save', exact: true })).toBeDisabled();
  await version.selectOption('8.4');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Startup recovery' })).toHaveCount(0);
  await expect(page.getByRole('status').filter({ hasText: 'Settings saved.' })).toBeVisible();
  await expect(version).toHaveValue('8.4');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeEnabled();
});

test('service startup switches are independent of the login item and live services', async ({ page }) => {
  await page.goto('/');
  const nginx = page.getByRole('switch', { name: 'Nginx Run on start' });
  await expect(page.getByRole('switch', { name: /Run on start$/ })).toHaveCount(3);
  await expect(nginx).not.toBeChecked();
  await nginx.check();
  await expect(nginx).toBeChecked();
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeEnabled();

  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const login = page.getByRole('switch', { name: 'Launch Fairway XMP at login' });
  await expect(login).not.toBeChecked();
  await login.check();
  await expect(login).toBeChecked();

  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(nginx).toBeChecked();
  await expect(page.getByRole('switch', { name: 'MySQL Run on start' })).not.toBeChecked();
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeEnabled();
});

test('service controls, terminals, and database navigation remain usable during download', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Update available');
  await page.evaluate(() => { (window as Window & { __fairwayUpdateDownloadDelay?: number }).__fairwayUpdateDownloadDelay = 10000; });
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await page.getByRole('button', { name: 'Install update', exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'Update download' })).toBeVisible();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Start all', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Stop all', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(page.getByLabel('Terminal input')).toBeVisible();
  await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Open SQLite file…', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'Update download' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Install update', exact: true })).toBeDisabled();
});
