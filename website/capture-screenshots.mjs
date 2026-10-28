// Run the app's simulated UI first: npm run dev:ui -- --port 1421
// No native services or user data are used by these captures.
import { chromium, expect } from '@playwright/test';
import { mkdir } from 'node:fs/promises';

const output = new URL('./assets/', import.meta.url);
const workspacesOnly = process.argv.includes('--workspaces-only');
const appGuidesOnly = process.argv.includes('--app-guides-only');
await mkdir(output, { recursive: true });
const browser = await chromium.launch();
try {
  const page = await browser.newPage({ viewport: { width: 1280, height: 960 }, deviceScaleFactor: 1 });
  await page.goto('http://127.0.0.1:1421/');
  await page.getByLabel('Example paths', { exact: true }).selectOption('macOS');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  await page.locator('.service-card .status-label.ready').first().waitFor();
  await page.addStyleTag({ content: '.ui-test-toolbar { display: none !important; }' });
  await page.evaluate(() => document.fonts.ready);
  if (!workspacesOnly && !appGuidesOnly) {
    await page.screenshot({ path: new URL('environment.png', output).pathname, animations: 'disabled' });
    await page.getByRole('button', { name: 'Browse Data', exact: true }).click();
    await page.getByRole('button', { name: 'Insert row', exact: true }).waitFor();
    await page.screenshot({ path: new URL('data-browser.png', output).pathname, animations: 'disabled' });
    await page.getByRole('button', { name: 'View Mail', exact: true }).click();
    await page.getByRole('list', { name: 'Emails' }).getByRole('button', { name: /Welcome to My Shop/ }).click();
    await page.frameLocator('iframe[title="Email content"]').getByRole('heading', { name: 'Welcome!' }).waitFor();
    await page.screenshot({ path: new URL('mail.png', output).pathname, animations: 'disabled' });
  }
  if (!appGuidesOnly) {
    await page.getByRole('button', { name: 'Terminal', exact: true }).click();
    await expect(page.getByRole('status').filter({ hasText: /^Connected$/ })).toBeVisible();
    const input = page.getByRole('textbox', { name: 'Terminal input', exact: true });
    for (const command of ['pwd', 'php --version', 'composer --version']) {
      await input.pressSequentially(command);
      await input.press('Enter');
    }
    await expect(page.locator('.xterm-rows')).toContainText('Composer version');
    await page.screenshot({ path: new URL('terminal.png', output).pathname, animations: 'disabled' });
    await page.getByRole('button', { name: 'Logs', exact: true }).click();
    await page.getByLabel('Log source', { exact: true }).selectOption('php');
    await expect(page.getByRole('region', { name: 'Log entries', exact: true })).toContainText('PHP Fatal error');
    await page.screenshot({ path: new URL('logs.png', output).pathname, animations: 'disabled' });
  }
  if (!workspacesOnly) {
    await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
    await expect(page.getByRole('list', { name: 'Saved sites' })).toBeVisible();
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.screenshot({ path: new URL('sites.png', output).pathname, animations: 'disabled' });
    await page.getByLabel('Scenario', { exact: true }).selectOption('Stopped', { force: true });
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByText('Advanced settings', { exact: true }).click();
    await expect(page.locator('.settings-card .advanced').first().locator('select')).toBeVisible();
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.screenshot({ path: new URL('settings.png', output).pathname, animations: 'disabled' });
    await page.getByLabel('Scenario', { exact: true }).selectOption('Update available', { force: true });
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Install update', exact: true })).toBeVisible();
    await page.evaluate(() => window.scrollTo(0, 0));
    await page.screenshot({ path: new URL('updates.png', output).pathname, animations: 'disabled' });
  }
  console.log('Captured documentation screenshots using simulated sample data.');
} finally {
  await browser.close();
}
