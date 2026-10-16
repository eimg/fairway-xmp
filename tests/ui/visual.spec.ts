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

test('projects are added, edited, and removed explicitly', async ({ page }, info) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('No projects');
  await expect(page.getByText('No projects yet.')).toBeVisible();
  await page.getByRole('button', { name: 'Add project' }).click();
  const dialog = page.getByRole('dialog', { name: 'Add project' });
  await dialog.getByLabel('Name').fill('Course Site');
  await page.locator('.modal-backdrop').click({ position: { x: 5, y: 5 } });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel('Domain')).toHaveValue('course-site');
  await expect(dialog.getByRole('button', { name: 'Add project' })).toBeDisabled();
  await dialog.getByLabel('Web root folder').click();
  await expect(dialog.getByLabel('Web root folder')).toHaveValue(/new-site.public$/);
  await expect(dialog).toContainText('course-site.test in your computer’s hosts file');
  await dialog.getByLabel('Route missing paths to index.php').check();
  await capture(page, info, 'add-project');
  await dialog.getByRole('button', { name: 'Add project' }).click();
  const table = page.getByRole('table', { name: 'Projects' });
  await expect(table).toContainText('Course Site');
  await expect(table.getByText('course-site.test:8800', { exact: true })).toBeVisible();

  await expect(page.getByRole('button', { name: 'Open Course Site' })).toBeDisabled();
  await page.getByRole('button', { name: 'Edit Course Site' }).click();
  const edit = page.getByRole('dialog', { name: 'Edit project' });
  await edit.getByLabel('Name').fill('Course');
  await expect(edit.getByLabel('Domain')).toHaveValue('course-site');
  await edit.getByLabel('Domain').fill('course');
  await expect(edit).toContainText('replaces course-site.test with course.test');
  await edit.getByRole('button', { name: 'Save changes' }).click();
  await expect(table.getByText('course.test:8800', { exact: true })).toBeVisible();

  await page.getByRole('button', { name: 'Remove Course' }).click();
  const remove = page.getByRole('dialog', { name: 'Remove Course?' });
  await expect(remove).toContainText('its files are kept');
  await remove.getByRole('button', { name: 'Remove project' }).click();
  await expect(page.getByText('No projects yet.')).toBeVisible();
});

test('removing the last HTTPS project also removes certificate trust', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Remove Blog' }).click();
  const blog = page.getByRole('dialog', { name: 'Remove Blog?' });
  await expect(blog).not.toContainText('certificate trust');
  await blog.getByRole('button', { name: 'Remove project' }).click();
  await page.getByRole('button', { name: 'Remove My Shop' }).click();
  const shop = page.getByRole('dialog', { name: 'Remove My Shop?' });
  await expect(shop).toContainText('also removes its local certificate trust');
  await shop.getByRole('button', { name: 'Remove project' }).click();
  await expect(page.getByText('No projects yet.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Remove certificate trust' })).toHaveCount(0);
});

test('invalid project details stay in the form with an error', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'Add project' }).click();
  const dialog = page.getByRole('dialog', { name: 'Add project' });
  await dialog.getByLabel('Name').fill('Shop');
  await dialog.getByRole('button', { name: 'Choose…' }).click();
  await dialog.getByRole('button', { name: 'Add project' }).click();
  await expect(dialog.getByRole('alert')).toContainText('shop.test is already used by another project.');
  await expect(dialog.getByLabel('Name')).toHaveValue('Shop');
});

test('HTTPS explains trust once and trust can be removed when unused', async ({ page }, info) => {
  await page.goto('/');
  const shop = page.getByRole('switch', { name: 'HTTPS for My Shop' });
  await expect(shop).toBeChecked();
  await shop.locator('..').click();
  await expect(shop).not.toBeChecked();
  await expect(page.getByRole('row', { name: /My Shop/ }).getByLabel('HTTPS', { exact: true })).toHaveCount(0);
  await page.getByRole('button', { name: 'Remove certificate trust' }).click();
  await expect(page.getByRole('button', { name: 'Remove certificate trust' })).toHaveCount(0);
  const blog = page.getByRole('switch', { name: 'HTTPS for Blog' });
  await blog.locator('..').click();
  const dialog = page.getByRole('dialog', { name: 'Enable HTTPS for blog.test?' });
  await expect(dialog).toContainText('only this computer trusts');
  await expect(dialog).toContainText('keeps working');
  await capture(page, info, 'enable-https');
  await dialog.getByRole('button', { name: 'Enable HTTPS' }).click();
  await expect(blog).toBeChecked();
  await expect(page.getByRole('row', { name: /Blog/ }).getByLabel('HTTPS', { exact: true })).toBeVisible();
  await shop.locator('..').click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(shop).toBeChecked();
});

test('pending hosts updates and missing folders are shown with a recovery action', async ({ page }, info) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Hosts update pending');
  await expect(page.getByText('Address not set up yet')).toBeVisible();
  await expect(page.getByRole('switch', { name: 'HTTPS for Blog' })).toBeDisabled();
  await capture(page, info, 'hosts-pending');
  await page.getByRole('button', { name: 'Set up addresses' }).click();
  await expect(page.getByText('Address not set up yet')).toHaveCount(0);
  await expect(page.getByRole('switch', { name: 'HTTPS for Blog' })).toBeEnabled();
  await page.getByLabel('Scenario', { exact: true }).selectOption('Missing web root');
  await expect(page.getByText('Unavailable: the web root folder is missing.', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Collapse projects' }).click();
  await expect(page.getByText('Unavailable: the web root folder is missing.', { exact: false })).toBeHidden();
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
