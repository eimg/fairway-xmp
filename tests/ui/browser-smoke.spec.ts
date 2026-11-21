import { expect, test } from '@playwright/test';

test('production preview cannot enable simulated services or create a shell', async ({ page }) => {
  await page.goto('http://127.0.0.1:1422/?ui-test=1');
  await expect(page.getByText('Interface preview — open the desktop app to manage services.')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toBeDisabled();
  await expect(page.getByRole('region', { name: 'UI test controls' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Terminal', exact: true }).click();
  await expect(page.getByText('Open the Fairway XMP desktop app to use the terminal.')).toBeVisible();
  await expect(page.locator('.terminal-host')).toHaveCount(0);
  await expect(page.getByLabel('New terminal', { exact: true })).toHaveCount(0);
});
