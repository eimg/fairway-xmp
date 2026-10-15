import { expect, test } from '@playwright/test';
import type { Page, TestInfo } from '@playwright/test';

const scenarios = ['Stopped', 'Running', 'Partially running', 'Port conflict', 'Missing runtime', 'Update available', 'Update failure'];

async function capture(page: Page, info: TestInfo, name: string) {
  await expect.poll(() => page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth)).toBe(true);
  const path = info.outputPath(`${name}.png`);
  await page.screenshot({ path, fullPage: true, animations: 'disabled' });
  await info.attach(name, { path, contentType: 'image/png' });
}

test('production browser preview cannot enable simulated services', async ({ page }) => {
  await page.goto('http://127.0.0.1:1422/?ui-test=1');
  await expect(page.getByText('Interface preview — open the desktop app to manage services.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeDisabled();
  await expect(page.getByRole('region', { name: 'UI test controls' })).toHaveCount(0);
});

test('project sites expand and HTTPS switch updates a site', async ({ page }, info) => {
  await page.goto('/');
  const projectSwitch = page.getByRole('switch', { name: 'HTTPS for hello' });
  await expect(projectSwitch).toBeVisible();
  await expect(projectSwitch).not.toBeChecked();
  await page.getByRole('button', { name: 'Collapse project sites' }).click();
  await expect(projectSwitch).toBeHidden();
  await page.getByRole('button', { name: 'Expand project sites' }).click();
  await expect(projectSwitch).toBeVisible();
  await projectSwitch.locator('..').click();
  await expect(projectSwitch).toBeChecked();
  await expect(page.getByText('https://hello.test:8443/')).toBeVisible();
  await capture(page, info, 'project-sites');
});

test('support report is reviewable and excludes private paths; old PHP versions are legacy', async ({ page }, info) => {
  await page.goto('/');
  await page.getByLabel('Example paths').selectOption('macOS');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Other PHP on PATH');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByText('Advanced settings', { exact: true }).click();
  await expect(page.getByRole('option', { name: 'PHP 8.0 · Legacy', exact: true })).toHaveCount(1);
  await expect(page.getByRole('option', { name: 'PHP 8.1 · Legacy', exact: true })).toHaveCount(1);
  await page.getByLabel('PHP version').selectOption('8.4');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Settings saved.' })).toBeVisible();
  await page.getByRole('button', { name: 'Diagnostic report', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Diagnostic report', exact: true });
  await expect(dialog).toBeVisible();
  const data = JSON.parse(await dialog.locator('pre').innerText());
  expect(data.commandLine.phpDetectedByApp).toBe('other-installation');
  expect(data.selectedPhp).toBe('8.4');
  expect(data.selectedPhpVersion).toMatch(/^8\.4\./);
  expect(data.selectedPhpLegacy).toBe(false);
  expect(data.system.os).toBe('macos');
  expect(data.services).toHaveLength(4);
  expect(JSON.stringify(data)).not.toContain('/Users/');
  expect(JSON.stringify(data)).not.toContain('/opt/homebrew');
  expect(data.excluded).toContain('credentials');
  await expect(dialog.getByRole('button', { name: 'Copy diagnostic report', exact: true })).toBeEnabled();
  await capture(page, info, 'diagnostic-report');
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

for (const scenario of scenarios) {
  test(`visual review: ${scenario}`, async ({ page }, info) => {
    const errors: string[] = [];
    page.on('pageerror', error => errors.push(error.message));
    await page.goto('/');
    await page.getByLabel('Example paths').selectOption('Windows');
    await page.getByLabel('Scenario', { exact: true }).selectOption(scenario);
    if (scenario === 'Running') await expect(page.getByText('All services ready', { exact: true })).toBeVisible();
    if (scenario === 'Partially running') await expect(page.getByText('Waiting', { exact: true })).toBeVisible();
    if (scenario === 'Missing runtime') await expect(page.getByRole('alert')).toContainText('files are missing');
    await capture(page, info, 'environment');
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    await page.getByText('Advanced settings', { exact: true }).click();
    await capture(page, info, 'settings-windows');
    await page.getByLabel('Example paths').selectOption('macOS');
    await expect(page.getByText('/Users/Student May/fairway/www', { exact: true })).toBeVisible();
    await capture(page, info, 'settings-macos');
    await page.getByRole('button', { name: 'Updates', exact: true }).click();
    await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
    await expect(page.getByRole('button', { name: 'Check for updates', exact: true })).toBeEnabled();
    if (scenario.startsWith('Update ')) {
      await expect(page.getByRole('button', { name: 'Install update', exact: true })).toBeVisible();
      if (scenario === 'Update failure') {
        await page.getByRole('button', { name: 'Install update', exact: true }).click();
        await expect(page.getByRole('alert')).toContainText('Could not download the update');
      }
    }
    await capture(page, info, 'updates');
    await page.getByRole('button', { name: 'Getting started' }).click();
    await expect(page.getByRole('dialog', { name: 'Your first PHP project' })).toBeVisible();
    await capture(page, info, 'help');
    await page.keyboard.press('Escape');
    await expect(page.getByRole('dialog')).toHaveCount(0);
    expect(errors).toEqual([]);
  });
}

test('interactive service, log, backup, and settings review', async ({ page }, info) => {
  await page.goto('/');
  const start = page.getByRole('button', { name: 'Start all', exact: true });
  await expect(start).toBeEnabled();
  await start.click();
  await expect(page.getByRole('status').filter({ hasText: 'Starting services…' })).toBeVisible();
  await capture(page, info, 'starting');
  await expect(page.getByText('All services ready', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'View Nginx logs', exact: true }).click();
  await expect(page.getByRole('dialog', { name: 'Nginx logs' })).toContainText('simulated nginx log');
  await capture(page, info, 'logs');
  await page.keyboard.press('Escape');
  await page.getByRole('button', { name: 'Back up databases', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Database backup saved:' })).toBeVisible();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Save', exact: true })).toBeDisabled();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'Stop all', exact: true }).click();
  await expect(page.getByText('All services stopped', { exact: true })).toBeVisible();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('spinbutton', { name: /^Website/ }).fill('8801');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Choose distinct ports');
  await capture(page, info, 'invalid-settings');
  await page.getByRole('spinbutton', { name: /^Website/ }).fill('8900');
  await page.getByText('Advanced settings', { exact: true }).click();
  await page.getByLabel('PHP version').selectOption('8.2');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Settings saved.' })).toBeVisible();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByText(':8900', { exact: true })).toBeVisible();
});

for (const platform of ['Windows', 'macOS']) {
  test(`CLI installation is distinct from app PATH detection: ${platform}`, async ({ page }, info) => {
    await page.goto('/');
    await page.getByLabel('Example paths').selectOption(platform);
    await page.getByLabel('Scenario', { exact: true }).selectOption('CLI not detected by app');
    await page.getByRole('button', { name: 'Settings', exact: true }).click();
    const tools = page.getByRole('region', { name: 'Command-line tools', exact: true });
    await expect(tools.getByText('Added to PATH', { exact: true })).toBeVisible();
    const php = tools.locator('.cli-paths dd').nth(0);
    const composer = tools.locator('.cli-paths dd').nth(1);
    const base = platform === 'Windows'
      ? 'C:\\Users\\Student May\\AppData\\Local\\dev.fairway.environment\\bin\\'
      : '/Users/Student May/Library/Application Support/dev.fairway.environment/bin/';
    await expect(php).toHaveText(`${base}${platform === 'Windows' ? 'php.cmd' : 'php'}`);
    await expect(composer).toHaveText(`${base}${platform === 'Windows' ? 'composer.cmd' : 'composer'}`);
    await expect(tools.getByText('Not found', { exact: true })).toHaveCount(0);
    await capture(page, info, 'cli-installed');
    await tools.getByText('Command detection details', { exact: true }).click();
    await expect(tools.getByText('Not detected in the app’s PATH', { exact: true })).toHaveCount(2);
    await capture(page, info, 'cli-detection-details');
    await page.getByLabel('Scenario', { exact: true }).selectOption('Other PHP on PATH');
    await expect(php).toHaveText(`${base}${platform === 'Windows' ? 'php.cmd' : 'php'}`);
    await expect(tools.getByText(platform === 'Windows' ? 'C:\\Other PHP\\php.exe' : '/opt/homebrew/bin/php', { exact: true })).toBeVisible();
    await capture(page, info, 'cli-other-installation');
    await tools.getByRole('button', { name: 'Remove from user PATH', exact: true }).click();
    await expect(tools.getByText('Not added to PATH', { exact: true })).toBeVisible();
    await expect(tools.locator('.cli-paths')).toHaveCount(0);
    await tools.getByRole('button', { name: 'Enable PHP and Composer', exact: true }).click();
    await expect(tools.getByText('Added to PATH', { exact: true })).toBeVisible();
    await expect(composer).toHaveText(`${base}${platform === 'Windows' ? 'composer.cmd' : 'composer'}`);
  });
}

test('port-conflict review and recovery', async ({ page }, info) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Port conflict');
  await page.getByRole('button', { name: 'Start all', exact: true }).click();
  await expect(page.getByRole('alert')).toContainText('Port 8800 is already in use');
  await capture(page, info, 'port-conflict');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('spinbutton', { name: /^Website/ }).fill('8900');
  await page.getByRole('button', { name: 'Save', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'Settings saved.' })).toBeVisible();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await page.getByRole('button', { name: 'Start all', exact: true }).click();
  await expect(page.getByText('All services ready', { exact: true })).toBeVisible();
});
