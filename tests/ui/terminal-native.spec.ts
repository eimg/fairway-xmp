import { test, expect } from '@playwright/test';
import { join } from 'node:path';
import { nativeBridge } from './native-bridge';

// Use the same production frontend that is packaged in the desktop app.
test.use({ baseURL: 'http://127.0.0.1:1422' });

// Opt-in after building the stdio example; ordinary browser CI uses the fixture.
// No listener is opened and the packaged runtime is read only.
test('React input reaches the real native PTY and packaged PHP/Composer', async ({ page }, info) => {
  test.skip(!process.env.FAIRWAY_TEST_RUNTIME, 'Set FAIRWAY_TEST_RUNTIME and build the terminal_bridge example.');
  const { close, errors, externalDirectories } = await nativeBridge(page, info);
  try {
    await page.goto('/');
    await page.getByRole('button', { name: 'Terminal', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
    const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
    const rows = page.locator('.terminal-tab-panel:not([hidden]) .terminal-host .xterm-rows');
    const enter = async (command: string) => { await input.focus(); await input.pressSequentially(command); await input.press('Enter'); };
    await expect(rows).toContainText('➜', { timeout: 15000 });
    await enter('php --version'); await expect(rows).toContainText(/PHP 8\.5\./);
    await enter('php --ri pdo_mysql'); await expect(rows).toContainText('PDO Driver for MySQL => enabled');
    await enter('composer --version'); await expect(rows).toContainText('Composer version');
    await enter(process.platform === 'win32' ? 'cd' : 'pwd');
    await expect(rows).toContainText(process.platform === 'win32' ? "Student May's home" : join('Student May\'s home 日本', 'fairway', 'www'));
    await enter('php -r "echo file_get_contents(\'.fairway-terminal-location\');"');
    await expect(rows).toContainText('COURSEWORK_FOLDER_VERIFIED');
    const viewport = page.locator('.terminal-tab-panel:not([hidden]) .xterm-viewport');
    await expect(viewport).toHaveCSS('background-color', 'rgb(23, 37, 31)');
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('button', { name: 'Dark', exact: true }).click();
    await page.getByRole('button', { name: 'Terminal', exact: true }).click();
    await expect(viewport).toHaveCSS('background-color', 'rgb(26, 27, 30)');
    await expect(rows).toContainText('Composer version');
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByRole('button', { name: 'Light', exact: true }).click();
    await page.getByRole('button', { name: 'Terminal', exact: true }).click();
    await expect(viewport).toHaveCSS('background-color', 'rgb(23, 37, 31)');
    await expect(rows).toContainText('Composer version');
    // Use a native executable on Windows so this checks PTY interruption
    // independently of Command Prompt's batch-file termination confirmation.
    await enter(process.platform === 'win32' ? 'echo FIRST_SERVER_READY & ping -t 127.0.0.1' : 'php -r "echo \'FIRST_SERVER_READY\'; sleep(30);"');
    await expect(rows).toContainText('FIRST_SERVER_READY');
    await page.getByLabel('New terminal', { exact: true }).click();
    await page.locator('.terminal-folder-menu').getByRole('button', { name: 'Project with spaces 日本', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
    await enter(process.platform === 'win32' ? 'cd' : 'pwd');
    await expect(rows).toContainText(process.platform === 'win32' ? 'Project with spaces' : 'Project with spaces 日本');
    await enter('php -r "echo file_get_contents(\'.fairway-terminal-location\');"');
    await expect(rows).toContainText('PROJECT_FOLDER_VERIFIED');
    await enter('cd ..');
    await enter(process.platform === 'win32' ? 'cd' : 'pwd');
    await page.getByRole('button', { name: 'Open in external terminal', exact: true }).click();
    await expect.poll(() => externalDirectories().at(-1)).toMatch(/[/\\]www$/);
    await enter('php -r "echo \'SECOND_SHELL_READY\';"');
    await expect(rows).toContainText('SECOND_SHELL_READY');
    await page.getByRole('tab', { name: 'Terminal 1', exact: true }).click();
    await expect(rows).toContainText('FIRST_SERVER_READY');
    await expect(rows).not.toContainText('SECOND_SHELL_READY');
    await input.press('Control+c');
    await enter('php -r "echo hex2bin(\'46495253545f5441425f524553554d4544\');"');
    await expect(rows).toContainText('FIRST_TAB_RESUMED');
    await page.setViewportSize({ width: 6000, height: 900 });
    await enter('echo native-resize-survived');
    await expect(rows).toContainText('native-resize-survived');
    await page.setViewportSize({ width: 820, height: 650 });
    await enter('exit 0');
    await expect(rows).not.toContainText('native-resize-survived');
    await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
    await page.getByRole('tab', { name: 'Terminal 2 · Project with spaces 日本', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
    await enter('composer --version'); await expect(rows).toContainText('Composer version');
    const path = info.outputPath('native-terminal.png');
    await page.screenshot({ path, fullPage: true }); await info.attach('native-terminal', { path, contentType: 'image/png' });
    await enter('exit 0');
    await expect(page.getByRole('tab')).toHaveCount(1);
    expect(errors()).toBe('');
  } finally {
    await close();
  }
});
