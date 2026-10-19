import { expect, test } from '@playwright/test';
import type { Page, TestInfo } from '@playwright/test';

const scenarios = ['Stopped', 'Running', 'Partially running', 'Port conflict', 'Missing runtime', 'Update available', 'Update failure'];

async function openSites(page: Page) {
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
}

async function siteAction(page: Page, name: string, action: string) {
  await page.getByRole('button', { name: `Actions for ${name}`, exact: true }).click();
  await page.getByRole('menuitem', { name: action, exact: true }).click();
}

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
  await openSites(page);
  await page.getByLabel('Scenario', { exact: true }).selectOption('No sites');
  await expect(page.getByText('No sites yet.')).toBeVisible();
  await page.getByRole('button', { name: 'Add site' }).click();
  const dialog = page.getByRole('dialog', { name: 'Add site' });
  await dialog.getByLabel('Site name').fill('Course Site');
  await page.locator('.modal-backdrop').click({ position: { x: 5, y: 5 } });
  await expect(dialog).toBeVisible();
  await expect(dialog.getByLabel('Domain', { exact: true })).toHaveValue('course-site');
  await expect(dialog.getByRole('button', { name: 'Add site' })).toBeDisabled();
  await dialog.getByLabel('Web root folder').click();
  await expect(dialog.getByLabel('Web root folder')).toHaveValue(/new-site.public$/);
  await dialog.getByRole('button', { name: 'About address permissions' }).click();
  await expect(dialog.getByRole('tooltip')).toContainText('course-site.test in your computer’s hosts file');
  await page.keyboard.press('Escape');
  await expect(dialog).toBeVisible();
  await expect(dialog.getByRole('tooltip')).toHaveCount(0);
  await dialog.getByLabel('App type').selectOption('front-controller');
  await expect(dialog.getByRole('button', { name: 'Cancel', exact: true })).toBeInViewport();
  await expect(dialog.getByRole('button', { name: 'Add site', exact: true })).toBeInViewport();
  await capture(page, info, 'add-project');
  await dialog.getByRole('button', { name: 'Add site' }).click();
  const table = page.getByRole('list', { name: 'Saved sites' });
  await expect(table).toContainText('Course Site');
  await expect(table.getByText('http://course-site.test:8800/', { exact: true })).toBeVisible();

  await expect(page.getByRole('button', { name: 'Open Course Site' })).toBeDisabled();
  await siteAction(page, 'Course Site', 'Edit site');
  const edit = page.getByRole('dialog', { name: 'Edit site' });
  await edit.getByLabel('Site name').fill('Course');
  await expect(edit.getByLabel('Domain', { exact: true })).toHaveValue('course-site');
  await edit.getByLabel('Domain', { exact: true }).fill('course');
  await edit.getByRole('button', { name: 'About address permissions' }).click();
  await expect(edit.getByRole('tooltip')).toContainText('replaces course-site.test with course.test');
  await page.keyboard.press('Escape');
  await edit.getByRole('button', { name: 'Save changes' }).click();
  await expect(table.getByText('http://course.test:8800/', { exact: true })).toBeVisible();

  await siteAction(page, 'Course', 'Remove site');
  const remove = page.getByRole('dialog', { name: 'Remove Course?' });
  await expect(remove).toContainText('its files are kept');
  await remove.getByRole('button', { name: 'Remove site' }).click();
  await expect(page.getByText('No sites yet.')).toBeVisible();
});

test('site form keeps detailed help optional and keyboard accessible', async ({ page }) => {
  await page.goto('/');
  await openSites(page);
  await page.getByRole('button', { name: 'Add site', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Add site' });
  await expect(dialog.getByRole('tooltip')).toHaveCount(0);
  await expect(dialog).not.toContainText('The site name can use any language');
  for (const label of ['About site domains', 'About the web root', 'About app routing', 'About address permissions']) {
    const button = dialog.getByRole('button', { name: label, exact: true });
    await button.focus();
    const tip = dialog.getByRole('tooltip');
    await expect(tip).toBeVisible();
    await expect(tip).toBeInViewport({ ratio: 1 });
    const fits = await tip.evaluate(element => {
      const bounds = element.getBoundingClientRect();
      const scroll = element.closest('.modal-form-body')!.getBoundingClientRect();
      return bounds.left >= scroll.left && bounds.right <= scroll.right && bounds.top >= scroll.top && bounds.bottom <= scroll.bottom;
    });
    expect(fits).toBe(true);
    await button.press('Enter'); // Help inside a form must never submit it.
    await expect(dialog).toBeVisible();
    await page.keyboard.press('Escape');
    await expect(tip).toHaveCount(0);
    await expect(dialog).toBeVisible();
  }
  await page.keyboard.press('Escape');
  await expect(dialog).toHaveCount(0);
});

test('removing the last HTTPS project also removes certificate trust', async ({ page }) => {
  await page.goto('/');
  await openSites(page);
  await siteAction(page, 'Blog', 'Remove site');
  const blog = page.getByRole('dialog', { name: 'Remove Blog?' });
  await expect(blog).not.toContainText('certificate trust');
  await blog.getByRole('button', { name: 'Remove site' }).click();
  await siteAction(page, 'My Shop', 'Remove site');
  const shop = page.getByRole('dialog', { name: 'Remove My Shop?' });
  await expect(shop).toContainText('also removes its local certificate trust');
  await shop.getByRole('button', { name: 'Remove site' }).click();
  await expect(page.getByText('No sites yet.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Remove certificate trust' })).toHaveCount(0);
});

test('invalid project details stay in the form with an error', async ({ page }) => {
  await page.goto('/');
  await openSites(page);
  await page.getByRole('button', { name: 'Add site' }).click();
  const dialog = page.getByRole('dialog', { name: 'Add site' });
  await dialog.getByLabel('Site name').fill('Shop');
  await dialog.getByRole('button', { name: 'Choose…' }).click();
  await dialog.getByRole('button', { name: 'Add site' }).click();
  await expect(dialog.getByRole('alert')).toContainText('shop.test is already used by another site.');
  await expect(dialog.getByLabel('Site name')).toHaveValue('Shop');
});

test('turning off the last HTTPS site removes trust, while other sites retain it', async ({ page }, info) => {
  await page.goto('/');
  await openSites(page);
  const shop = page.getByRole('switch', { name: 'HTTPS for My Shop' });
  await expect(shop).toBeChecked();
  await shop.locator('..').click();
  await expect(shop).not.toBeChecked();
  await expect(page.getByRole('listitem', { name: 'My Shop', exact: true }).getByLabel('HTTPS', { exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Remove certificate trust' })).toHaveCount(0);
  await expect(page.getByRole('status')).toContainText('Its unused certificate trust was removed');
  const blog = page.getByRole('switch', { name: 'HTTPS for Blog' });
  await blog.locator('..').click();
  const dialog = page.getByRole('dialog', { name: 'Enable HTTPS for blog.test?' });
  await expect(dialog).toContainText('only this computer trusts');
  await expect(dialog).toContainText('Turning off HTTPS for the last site also removes the shared certificate trust');
  await capture(page, info, 'enable-https');
  await dialog.getByRole('button', { name: 'Enable HTTPS' }).click();
  await expect(blog).toBeChecked();
  await expect(page.getByRole('listitem', { name: 'Blog', exact: true }).getByLabel('HTTPS', { exact: true })).toBeVisible();
  await shop.locator('..').click();
  await expect(page.getByRole('dialog')).toHaveCount(0);
  await expect(shop).toBeChecked();
  await shop.locator('..').click();
  await expect(shop).not.toBeChecked();
  await expect(blog).toBeChecked();
  // The CA still serves Blog: enabling My Shop again must not ask for trust.
  await shop.locator('..').click();
  await expect(shop).toBeChecked();
  await expect(page.getByRole('dialog')).toHaveCount(0);
});

test('pending hosts updates and missing folders are shown with a recovery action', async ({ page }, info) => {
  await page.goto('/');
  await openSites(page);
  await page.getByLabel('Scenario', { exact: true }).selectOption('Hosts update pending');
  await expect(page.getByText('Address setup needed', { exact: true })).toBeVisible();
  await expect(page.getByRole('switch', { name: 'HTTPS for Blog' })).toBeDisabled();
  await capture(page, info, 'hosts-pending');
  await page.getByRole('button', { name: 'Set up addresses' }).click();
  await expect(page.getByText('Address setup needed', { exact: true })).toHaveCount(0);
  await expect(page.getByRole('switch', { name: 'HTTPS for Blog' })).toBeEnabled();
  await page.getByLabel('Scenario', { exact: true }).selectOption('Missing web root');
  await expect(page.getByText('The web root folder is missing.', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByText('The web root folder is missing.', { exact: false })).toBeHidden();
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
  expect(data.services).toHaveLength(5);
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
  await page.getByRole('button', { name: 'Logs', exact: true }).click();
  await expect(page.getByRole('region', { name: 'Log entries' })).toContainText('PHP Fatal error');
  await capture(page, info, 'logs');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
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


test('site readiness starts only the web server and secure edits keep their URL', async ({ page }, info) => {
  await page.goto('/'); await openSites(page);
  await expect(page.getByRole('button', { name: 'Open My Shop', exact: true })).toBeDisabled();
  await expect(page.getByRole('listitem', { name: 'My Shop', exact: true })).toContainText('Web server stopped');
  await siteAction(page, 'My Shop', 'Edit site');
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'My Shop saved. Web server stopped.' })).toBeVisible();
  await page.getByRole('button', { name: 'Start web server', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Open My Shop', exact: true })).toBeEnabled();
  await siteAction(page, 'My Shop', 'Edit site');
  await page.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.getByRole('status').filter({ hasText: 'My Shop is ready at https://shop.test:8443/' })).toBeVisible();
  expect(await page.locator('.site-item').evaluateAll(items => items.every(item => item.scrollWidth <= item.clientWidth))).toBe(true);
  await capture(page, info, 'sites-ready');
  await page.getByRole('button', { name: 'Actions for My Shop', exact: true }).click();
  await capture(page, info, 'site-actions');
  await page.getByRole('menuitem', { name: 'Open folder', exact: true }).press('Escape');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.getByRole('heading', { name: 'MySQL', exact: true }).locator('..').locator('..')).toContainText('Stopped');
});

test('saved-list errors never look like an empty site list and folder guidance is conditional', async ({ page }, info) => {
  await page.goto('/'); await openSites(page);
  await page.getByLabel('Scenario', { exact: true }).selectOption('Saved sites unavailable');
  await expect(page.getByRole('alert')).toContainText('Your saved sites couldn’t be loaded');
  await expect(page.getByText('No sites yet.', { exact: true })).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Add site', exact: true })).toBeDisabled();
  await expect(page.getByRole('button', { name: 'Retry loading sites', exact: true })).toBeEnabled();
  await capture(page, info, 'sites-load-error');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Stopped');
  await siteAction(page, 'Blog', 'Edit site');
  const dialog = page.getByRole('dialog');
  await expect(dialog).toContainText('Also available through localhost.');
  await dialog.getByRole('button', { name: 'About shared document-root access' }).focus();
  await expect(dialog.getByRole('tooltip')).toContainText('Other project files may be accessible');
  await expect(dialog.getByRole('tooltip')).toBeInViewport({ ratio: 1 });
  await page.keyboard.press('Escape');
  await expect(dialog).toBeVisible();
  await page.getByRole('button', { name: 'Cancel', exact: true }).click();
  await siteAction(page, 'My Shop', 'Edit site');
  await expect(page.getByRole('dialog')).not.toContainText('Also available through localhost.');
  await expect(page.getByRole('button', { name: 'About shared document-root access' })).toHaveCount(0);
});

test('site terminals open registered locations without replacing existing sessions', async ({ page }) => {
  await page.goto('/'); await openSites(page);
  await siteAction(page, 'My Shop', 'Terminal in app folder');
  const first = page.getByRole('tab', { name: 'Terminal 1 · My Shop · app folder', exact: true });
  await expect(first).toHaveAttribute('aria-selected', 'true');
  await expect(page.locator('.terminal-directory code')).toHaveText(/Code[\\/]shop$/);
  await expect(page.getByText('Connected', { exact: true })).toBeVisible();
  await openSites(page); await siteAction(page, 'My Shop', 'Terminal in web root');
  await expect(page.getByRole('tab', { name: 'Terminal 2 · My Shop · web root', exact: true })).toHaveAttribute('aria-selected', 'true');
  await expect(page.getByText('Connected', { exact: true })).toBeVisible();
  await openSites(page); await siteAction(page, 'Blog', 'Terminal in web root');
  await expect(page.getByText('Both terminal tabs are open.', { exact: false })).toBeVisible();
  await expect(first).toBeVisible();
  await expect(page.getByRole('tab')).toHaveCount(2);
});


test('copy site URL includes HTTPS and the port', async ({ page, context, browserName }) => {
  test.skip(browserName !== 'chromium', 'Clipboard permission grants are available in Chromium.');
  await context.grantPermissions(['clipboard-read', 'clipboard-write']);
  await page.goto('/'); await openSites(page);
  await page.getByRole('button', { name: 'Copy URL for My Shop', exact: true }).click();
  await expect.poll(() => page.evaluate(() => navigator.clipboard.readText())).toBe('https://shop.test:8443/');
});


test('update progress clears after a failed download and services stay available', async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Update failure');
  await page.getByRole('button', { name: 'Start all', exact: true }).click();
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await page.getByRole('button', { name: 'Check for updates', exact: true }).click();
  await page.getByRole('button', { name: 'Install update', exact: true }).click();
  await expect(page.getByRole('progressbar', { name: 'Update download' })).toHaveAttribute('value', '25');
  await expect(page.getByRole('button', { name: 'Install update', exact: true })).toBeDisabled();
  await expect(page.getByRole('alert')).toContainText('Your services are still available');
  await expect(page.getByRole('progressbar')).toHaveCount(0);
  await expect(page.getByRole('button', { name: 'Install update', exact: true })).toBeEnabled();
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.locator('.service-card .status-label.ready')).toHaveCount(5);
});

test('mail inbox lists, previews, searches, and deletes caught emails', async ({ page }, info) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await page.getByRole('button', { name: 'SMTP settings' }).click();
  const connection = page.getByRole('dialog', { name: 'Mail connection' });
  await expect(connection).toContainText('11025');
  await expect(connection.getByRole('button', { name: 'Copy Laravel mail settings' })).toBeVisible();
  await capture(page, info, 'mail-connection');
  await connection.getByRole('button', { name: 'Close dialog' }).click();
  const emails = page.getByRole('list', { name: 'Emails' });
  await expect(emails.getByRole('button')).toHaveCount(3);
  await expect(page.getByText('3 emails · 1 unread')).toBeVisible();
  await emails.getByRole('button', { name: /Welcome to My Shop/ }).click();
  await expect(page.getByRole('heading', { name: 'Welcome to My Shop' })).toBeVisible();
  const body = page.frameLocator('iframe[title="Email content"]');
  await expect(body.getByRole('heading', { name: 'Welcome!' })).toBeVisible();
  await expect(body.locator('body')).not.toContainText('scripts must not run');
  await expect(page.locator('iframe[title="Email content"]')).toHaveAttribute('sandbox', '');
  await expect(page.getByText('3 emails', { exact: true })).toBeVisible();
  await capture(page, info, 'mail');
  await page.getByRole('tab', { name: 'Text' }).click();
  await expect(page.locator('pre.mail-text')).toHaveText('Thanks for joining My Shop.');
  await emails.getByRole('button', { name: /Invoice #1042/ }).click();
  await page.getByRole('list', { name: 'Attachments' }).getByRole('button', { name: /invoice-1042\.pdf/ }).click();
  await expect(page.getByText('Saved to /Users/you/Downloads/invoice-1042.pdf')).toBeVisible();
  await page.getByLabel('Search emails').fill('reset');
  await expect(emails.getByRole('button')).toHaveCount(1);
  await page.getByLabel('Search emails').fill('');
  await expect(emails.getByRole('button')).toHaveCount(3);
  await emails.getByRole('button', { name: /Reset your password/ }).click();
  await page.getByRole('button', { name: 'Delete email' }).click();
  await expect(emails.getByRole('button')).toHaveCount(2);
  await page.getByRole('button', { name: 'Delete all' }).click();
  await page.getByRole('button', { name: 'Delete all' }).click();
  await expect(page.getByText('No emails yet.', { exact: false })).toBeVisible();
  await page.getByRole('button', { name: 'Stop mail' }).click();
  await expect(page.getByRole('heading', { name: 'Mail is stopped.' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Open in browser' })).toBeDisabled();
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await expect(page.getByLabel(/Mail inbox/)).toHaveValue('8802');
  await expect(page.getByLabel(/Mail SMTP/)).toHaveValue('11025');
});
