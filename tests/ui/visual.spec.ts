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
  await expect(page.locator('.sidebar')).not.toContainText('Made for local learning');
  await expect(page.locator('.sidebar-footer')).toHaveText(/^v\d+\.\d+\.\d+/);
  await expect(page.locator('.sidebar-footer')).not.toContainText('FAIRWAY XMP');
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
  await expect(page.getByRole('dialog', { name: 'Enable HTTPS for hello.test?' })).toContainText('trust store');
  await page.getByRole('button', { name: 'Enable HTTPS', exact: true }).click();
  await expect(projectSwitch).toBeChecked();
  await expect(page.getByText('hello.test', { exact: true })).toHaveAttribute('title', 'https://hello.test:8443/');
  await capture(page, info, 'project-sites');
});

test('project domains are opt-in and independent', async ({ page }, info) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Domains not set up');
  await expect(page.getByRole('table', { name: 'Project sites' })).toBeVisible();
  await expect(page.getByText('127.0.0.1:8800/hello/')).toHaveAttribute('title', 'http://127.0.0.1:8800/hello/');
  await expect(page.getByText('Uses project domain')).toBeVisible();
  await expect(page.getByRole('switch', { name: 'HTTPS for hello' })).toBeDisabled();
  await page.getByRole('button', { name: 'Refresh projects' }).click();
  await expect(page.getByText('hello.test', { exact: true })).toHaveClass(/project-domain-muted/);
  await page.getByRole('button', { name: 'Enable hello.test' }).click();
  const dialog = page.getByRole('dialog', { name: 'Enable hello.test?' });
  await expect(dialog).toContainText('system hosts file');
  await dialog.getByRole('button', { name: 'Enable', exact: true }).click();
  await expect(page.getByText('hello.test', { exact: true })).toHaveClass(/project-domain-active/);
  await expect(page.getByText('127.0.0.1:8800/hello/')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Enable laravel-app.test' })).toBeVisible();
  await capture(page, info, 'one-domain-enabled');
  await page.getByRole('button', { name: 'Enable laravel-app.test' }).click();
  await page.getByRole('dialog', { name: 'Enable laravel-app.test?' }).getByRole('button', { name: 'Enable', exact: true }).click();
  await expect(page.getByText('laravel-app.test', { exact: true })).toHaveClass(/project-domain-active/);
  await page.getByRole('button', { name: 'Disable hello.test' }).click();
  await page.getByRole('dialog', { name: 'Disable hello.test?' }).getByRole('button', { name: 'Disable', exact: true }).click();
  await expect(page.getByText('127.0.0.1:8800/hello/')).toBeVisible();
  await expect(page.getByText('hello.test', { exact: true })).toHaveClass(/project-domain-muted/);
  await expect(page.getByText('laravel-app.test', { exact: true })).toHaveClass(/project-domain-active/);
});

test('an address remains removable after its project folder is deleted', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Removed project domain');
  await expect(page.getByText('This managed address has no project folder.')).toBeVisible();
  await page.getByRole('button', { name: 'Remove unused .test for old-project' }).click();
  await page.getByRole('dialog', { name: 'Disable old-project.test?' }).getByRole('button', { name: 'Disable', exact: true }).click();
  await expect(page.getByText('This managed address has no project folder.')).toHaveCount(0);
  await expect(page.getByText('hello.test', { exact: true })).toHaveClass(/project-domain-active/);
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
    await expect(tools.getByText('Database commands', { exact: true })).not.toBeVisible();
    await expect(tools.locator('.cli-commands code').filter({ hasText: 'fairway db' })).toHaveCount(1);
    await capture(page, info, 'cli-compact');
    await tools.getByRole('button', { name: 'Command-line tools help', exact: true }).click();
    const tooltip = page.getByRole('tooltip');
    await expect(tooltip).toContainText('Open a new terminal');
    await expect(tooltip.locator('code').filter({ hasText: 'fairway db --help' })).toHaveCount(1);
    await capture(page, info, 'cli-help');
    await page.keyboard.press('Escape');
    await expect(tooltip).toHaveCount(0);
    await tools.getByText('Command details', { exact: true }).click();
    const php = tools.locator('.cli-paths dd').nth(0);
    const composer = tools.locator('.cli-paths dd').nth(1);
    const base = platform === 'Windows'
      ? 'C:\\Users\\Student May\\AppData\\Local\\dev.fairway.environment\\bin\\'
      : '/Users/Student May/Library/Application Support/dev.fairway.environment/bin/';
    await expect(php).toHaveText(`${base}${platform === 'Windows' ? 'php.cmd' : 'php'}`);
    await expect(composer).toHaveText(`${base}${platform === 'Windows' ? 'composer.cmd' : 'composer'}`);
    await expect(tools.getByText('Database commands', { exact: true })).toHaveCount(1);
    await expect(tools.getByText('Not found', { exact: true })).toHaveCount(0);
    await capture(page, info, 'cli-installed');
    await expect(tools.getByText('Not detected in the app’s PATH', { exact: true })).toHaveCount(2);
    await capture(page, info, 'cli-detection-details');
    await page.getByLabel('Scenario', { exact: true }).selectOption('Other PHP on PATH');
    await expect(php).toHaveText(`${base}${platform === 'Windows' ? 'php.cmd' : 'php'}`);
    await expect(tools.getByText(platform === 'Windows' ? 'C:\\Other PHP\\php.exe' : '/opt/homebrew/bin/php', { exact: true })).toBeVisible();
    await capture(page, info, 'cli-other-installation');
    await tools.getByRole('button', { name: 'Remove from user PATH', exact: true }).click();
    await expect(tools.getByText('Not added to PATH', { exact: true })).toBeVisible();
    await expect(tools.locator('.cli-paths')).toHaveCount(0);
    await tools.getByRole('button', { name: 'Enable command-line tools', exact: true }).click();
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
