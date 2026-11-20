import { expect, test } from '@playwright/test';
import { readFile } from 'node:fs/promises';
import { join } from 'node:path';
import { nativeBridge } from './native-bridge';

test.use({ baseURL: 'http://127.0.0.1:1422' });

test('PHP settings UI saves real persistent overrides and regenerates native configs', async ({ page }, info) => {
  test.skip(!process.env.FAIRWAY_TEST_RUNTIME, 'Set FAIRWAY_TEST_RUNTIME and build terminal_bridge.');
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/');
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    const card = page.getByRole('region', { name: 'PHP settings', exact: true });
    await card.getByRole('checkbox', { name: /^Session upload progress/ }).check();
    await card.getByLabel(/^Upload size per file/).fill('128');
    await card.getByLabel(/^Timezone/).fill('Asia/Yangon');
    await card.getByRole('button', { name: 'Save PHP settings', exact: true }).click();
    await expect(card.getByText('Custom settings saved for this version.')).toBeVisible();
    const data = join(bridge.root, 'data 日本');
    const saved = JSON.parse(await readFile(join(data, 'php-settings.json'), 'utf8'));
    expect(saved.versions['8.5']).toEqual({ sessionUploadProgress: true, timezone: 'Asia/Yangon', uploadMaxFilesizeMb: 128 });
    const ini = await readFile(join(data, 'config/php.ini'), 'utf8');
    expect(ini).toContain('session.upload_progress.enabled=On\n');
    expect(ini).toContain('post_max_size=136M\n');
    expect(await readFile(join(data, 'config/nginx.conf'), 'utf8')).toContain('client_max_body_size 136m;');
    await page.reload();
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await expect(card.getByRole('checkbox', { name: /^Session upload progress/ })).toBeChecked();
    await card.getByRole('button', { name: 'Reset to defaults', exact: true }).click();
    await expect(card.getByRole('checkbox', { name: /^Session upload progress/ })).not.toBeChecked();
    expect(JSON.parse(await readFile(join(data, 'php-settings.json'), 'utf8')).versions).toEqual({});
    expect(bridge.errors()).toBe('');
  } finally { await bridge.close(); }
});
