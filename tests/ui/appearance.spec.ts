import { expect, test } from '@playwright/test';

test('desktop appearance defaults to Light and remembers a manual choice', async ({ page }) => {
  await page.emulateMedia({ colorScheme: 'dark' });
  await page.goto('/');

  const openSettings = () => page.getByRole('button', { name: 'Settings', exact: true }).click();
  const appearance = page.getByRole('group', { name: 'Appearance', exact: true });
  const light = appearance.getByRole('button', { name: 'Light', exact: true });
  const dark = appearance.getByRole('button', { name: 'Dark', exact: true });
  const root = page.locator('html');
  const sidebar = page.locator('.sidebar');

  await openSettings();
  await expect(light).toHaveAttribute('aria-pressed', 'true');
  await expect(appearance.getByRole('button')).toHaveCount(2);
  await expect(root).toHaveCSS('color-scheme', 'light');
  await expect(sidebar).toHaveCSS('background-color', 'rgb(255, 255, 255)');

  await dark.press('Enter');
  await expect(dark).toHaveAttribute('aria-pressed', 'true');
  await expect(root).toHaveCSS('color-scheme', 'dark');
  await expect(root).toHaveCSS('background-color', 'rgb(23, 24, 26)');
  await expect(sidebar).toHaveCSS('background-color', 'rgb(32, 33, 36)');
  await page.getByRole('button', { name: 'Environment', exact: true }).click();
  await expect(page.locator('.service-card').first()).toHaveCSS('background-color', 'rgb(32, 33, 36)');
  await expect(page.getByRole('button', { name: 'Start all', exact: true })).toHaveCSS('background-color', 'rgb(40, 109, 85)');

  await page.emulateMedia({ colorScheme: 'light' });
  await page.reload();
  await expect(root).toHaveCSS('color-scheme', 'dark');
  await openSettings();
  await expect(dark).toHaveAttribute('aria-pressed', 'true');
  await light.click();
  await page.reload();
  await expect(root).toHaveCSS('color-scheme', 'light');
});
