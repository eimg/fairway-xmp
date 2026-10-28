// Requires the documentation preview; both themes are tested in real browser engines.
import assert from 'node:assert/strict';
import test from 'node:test';
import { chromium, webkit, expect } from '@playwright/test';
import { copy } from './content.mjs';

const base = process.env.SITE_BASE_URL || 'http://127.0.0.1:4173/fairway-xmp/';
const background = page => page.locator('body').evaluate(element => getComputedStyle(element).backgroundColor);
const colors = { light: 'rgb(247, 248, 242)', dark: 'rgb(18, 28, 24)' };

for (const [name, engine] of Object.entries({ chromium, webkit })) {
  test(`${name}: system changes, explicit overrides, reloads and language switching`, async () => {
    const browser = await engine.launch();
    try {
      const context = await browser.newContext({ colorScheme: 'dark', reducedMotion: 'reduce' });
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      await page.goto(base);
      const toggle = page.getByRole('switch');
      await expect(toggle).toHaveAccessibleName(copy.en.theme.label);
      await expect(toggle).toBeChecked();
      await expect.poll(() => background(page)).toBe(colors.dark);
      await page.emulateMedia({ colorScheme: 'light' });
      await expect.poll(() => background(page)).toBe(colors.light);
      await expect(toggle).not.toBeChecked();
      assert.equal(await page.evaluate(() => localStorage.getItem('fairway-docs-theme')), null);
      // Native button keyboard activation sets a persistent explicit preference.
      await toggle.focus();
      await page.keyboard.press('Space');
      await page.reload();
      await expect(toggle).toBeChecked();
      await expect.poll(() => background(page)).toBe(colors.dark);
      await page.locator('.languages a[lang="my"]').click();
      await expect(toggle).toHaveAccessibleName(copy.mm.theme.label);
      await expect(toggle).toBeChecked();
      await expect.poll(() => background(page)).toBe(colors.dark);
      const other = await page.context().newPage();
      await other.goto(base);
      await other.getByRole('switch').click();
      await expect.poll(() => background(page)).toBe(colors.light);
      await expect(toggle).not.toBeChecked();
      await other.close();
      await page.emulateMedia({ colorScheme: 'dark' });
      await expect.poll(() => background(page)).toBe(colors.light);
      await page.reload();
      await expect(toggle).not.toBeChecked();
      await expect.poll(() => background(page)).toBe(colors.light);
      // Clearing a preference returns to the automatic system fallback.
      await page.evaluate(() => localStorage.removeItem('fairway-docs-theme'));
      await page.reload();
      await expect.poll(() => background(page)).toBe(colors.dark);
      await page.emulateMedia({ colorScheme: 'light' });
      await expect.poll(() => background(page)).toBe(colors.light);
      await expect(page.locator('meta[name="theme-color"]')).toHaveAttribute('content', '#f7f8f2');
      assert.deepEqual(errors, []);
    } finally { await browser.close(); }
  });

  test(`${name}: system fallback without JavaScript and manual choice without storage`, async () => {
    const browser = await engine.launch();
    try {
      const context = await browser.newContext({ javaScriptEnabled: false, colorScheme: 'dark' });
      const page = await context.newPage();
      await page.goto(base);
      await expect(page.getByRole('switch')).toBeHidden();
      await expect.poll(() => background(page)).toBe(colors.dark);
      await page.emulateMedia({ colorScheme: 'light' });
      await expect.poll(() => background(page)).toBe(colors.light);
      await context.close();

      const blocked = await browser.newContext({ colorScheme: 'dark' });
      await blocked.addInitScript(() => {
        for (const name of ['getItem', 'setItem']) Object.defineProperty(Storage.prototype, name, {
          value() { throw new DOMException('Storage blocked', 'SecurityError'); },
        });
      });
      const restricted = await blocked.newPage();
      await restricted.goto(base);
      await expect.poll(() => background(restricted)).toBe(colors.dark);
      await restricted.getByRole('switch').click();
      await expect.poll(() => background(restricted)).toBe(colors.light);
      // The theme fallback must not prevent other page controls from starting.
      await restricted.locator('[data-open-downloads]').click();
      await expect(restricted.locator('#download-dialog')).toBeVisible();
    } finally { await browser.close(); }
  });

  test(`${name}: readable themes and responsive controls in both languages`, async () => {
    const browser = await engine.launch();
    const luminance = value => {
      const rgb = value.match(/[\d.]+/g).slice(0, 3).map(Number).map(channel => {
        const normalized = channel / 255;
        return normalized <= .04045 ? normalized / 12.92 : ((normalized + .055) / 1.055) ** 2.4;
      });
      return .2126 * rgb[0] + .7152 * rgb[1] + .0722 * rgb[2];
    };
    try {
      const page = await browser.newPage({ reducedMotion: 'reduce' });
      for (const route of ['', 'mm/']) {
        await page.goto(`${base}${route}`);
        for (const width of [1280, 390, 320]) {
          await page.setViewportSize({ width, height: 900 });
          for (const mode of ['light', 'dark']) {
            const toggle = page.getByRole('switch');
            if ((await toggle.getAttribute('aria-checked')) !== String(mode === 'dark')) await toggle.click();
            await expect.poll(() => background(page)).toBe(colors[mode]);
            const pairs = await page.evaluate(() => {
              const selectors = ['body', '.docs-content p', '.guide-steps', '.common-tasks a', '.guide-result p', '.guide-result h3', '.guide-platform summary', '[data-theme-toggle]', 'thead', '.download-main-button'];
              const backgroundFor = element => {
                for (let node = element; node; node = node.parentElement) {
                  const color = getComputedStyle(node).backgroundColor;
                  if (color !== 'rgba(0, 0, 0, 0)') return color;
                }
              };
              const elements = selectors.map(selector => document.querySelector(selector));
              elements.push(document.querySelector('#mysql pre code'), document.querySelector('#nginx li > code'));
              return elements.map(element => ({ text: getComputedStyle(element).color, background: backgroundFor(element), tag: element.tagName }));
            });
            for (const pair of pairs) {
              const values = [luminance(pair.text), luminance(pair.background)].sort((a, b) => b - a);
              const contrast = (values[0] + .05) / (values[1] + .05);
              assert(contrast >= 4.5, `${name}/${route}/${mode}/${width}: ${pair.tag} contrast ${contrast.toFixed(2)}`);
            }
            assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${route}/${mode}: no overflow at ${width}px`);
            await page.locator('[data-open-downloads]').click();
            await expect(page.locator('#download-dialog')).toBeVisible();
            await page.keyboard.press('Escape');
            await expect(page.locator('[data-open-downloads]')).toBeFocused();
          }
        }
      }
    } finally { await browser.close(); }
  });
}
