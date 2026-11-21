import { test, expect } from '@playwright/test';
import { nativeBridge } from './native-bridge';

// Exercise the built frontend through a real PTY, without a browser fixture.
test.use({ baseURL: 'http://127.0.0.1:1422' });

test('terminal input reaches the native PTY and bundled PHP/Composer', async ({ page }, info) => {
  const bridge = await nativeBridge(page, info);
  try {
    await page.goto('/');
    await page.getByRole('button', { name: 'Terminal', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
    const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
    const rows = page.locator('.terminal-tab-panel:not([hidden]) .terminal-host .xterm-rows');
    const enter = async (command: string) => { await input.focus(); await input.pressSequentially(command); await input.press('Enter'); };
    await expect(rows).toContainText('➜', { timeout: 15000 });
    await enter('php --version');
    await expect(rows).toContainText(/PHP \d+\.\d+\.\d+/);
    await enter('composer --version');
    await expect(rows).toContainText('Composer version');
    // The encoded marker cannot pass merely because the shell echoes its input.
    await enter(`php -r "echo hex2bin('4e41544956455f5054595f5645524946494544');"`);
    await expect(rows).toContainText('NATIVE_PTY_VERIFIED');
    expect(bridge.errors()).toBe('');
  } finally { await bridge.close(); }
});
