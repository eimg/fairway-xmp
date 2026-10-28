// Run with npm run site:dev serving the documentation in another terminal.
import assert from 'node:assert/strict';
import test from 'node:test';
import { chromium, webkit, expect } from '@playwright/test';

const base = process.env.SITE_BASE_URL || 'http://127.0.0.1:4173/fairway-xmp/';

for (const [name, engine] of Object.entries({ chromium, webkit })) {
  test(`${name}: language navigation follows the visible chapter, not a stale hash`, async () => {
    const browser = await engine.launch();
    try {
      const page = await browser.newPage();
      await page.emulateMedia({ reducedMotion: 'reduce' });
      for (const width of [1280, 390]) {
        await page.setViewportSize({ width, height: 900 });
        for (const [route, language, translated] of [['', 'my', 'mm/'], ['mm/', 'en', '']]) {
          await page.goto(`${base}${route}#logs`);
          await page.evaluate(() => {
            const section = document.querySelector('#logs');
            window.scrollTo({ top: section.getBoundingClientRect().top + scrollY - 44, behavior: 'instant' });
          });
          const link = page.locator(`.languages a[lang="${language}"]`);
          await expect.poll(async () => new URL(await link.getAttribute('href'), page.url()).hash).toBe('#logs');

          // Reading another chapter without clicking the TOC leaves #logs in the URL.
          for (const chapter of ['terminal', 'sites', 'settings', 'updates']) {
            await page.evaluate(id => {
              const section = document.getElementById(id);
              window.scrollTo({ top: section.getBoundingClientRect().top + scrollY - 44, behavior: 'instant' });
            }, chapter);
            assert.equal(new URL(page.url()).hash, '#logs');
            await expect.poll(async () => new URL(await link.getAttribute('href'), page.url()).hash).toBe(`#${chapter}`);
          }

          // Returning to the header must clear the old chapter before switching.
          await page.evaluate(() => window.scrollTo({ top: 0, behavior: 'instant' }));
          await expect.poll(async () => new URL(await link.getAttribute('href'), page.url()).hash).toBe('');
          await link.click();
          assert.equal(page.url(), `${base}${translated}`);
          await expect.poll(() => page.evaluate(() => window.scrollY)).toBe(0);
        }
      }
    } finally {
      await browser.close();
    }
  });
}

for (const [name, engine] of Object.entries({ chromium, webkit })) {
  test(`${name}: tasks and contextual help are usable on desktop and mobile without JavaScript`, async () => {
    const browser = await engine.launch();
    try {
      const context = await browser.newContext({ javaScriptEnabled: false, reducedMotion: 'reduce' });
      const page = await context.newPage();
      for (const width of [1280, 390]) {
        await page.setViewportSize({ width, height: 900 });
        for (const route of ['', 'mm/']) {
          await page.goto(`${base}${route}`);
          await expect(page.locator('[data-download-fallback]')).toBeVisible();
          await expect(page.locator('.mobile-contents')).toBeVisible({ visible: width < 760 });
          await page.locator('.common-tasks a[href="#sites"]').click();
          assert.equal(new URL(page.url()).hash, '#sites');
          await expect(page.locator('#sites .guide-steps > li')).toHaveCount(5);
          const help = page.locator('#sites .guide-troubleshooting details').first();
          await help.locator('summary').click();
          await expect(help.locator('p')).toBeVisible();
          await page.locator('#sites .guide-related a[href="#mysql"]').click();
          await expect(page.locator('#mysql pre')).toContainText('DB_HOST=127.0.0.1');
          if (width < 760) {
            // The mobile directory stays available without client-side code.
            await page.goto(`${base}${route}`);
            await page.locator('.mobile-contents > summary').click();
            await page.locator('.mobile-contents a[href="#node"]').click();
            assert.equal(new URL(page.url()).hash, '#node');
            await expect(page.locator('#node .guide-result')).toBeVisible();
          }
          assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${route}: no horizontal page overflow at ${width}px`);
        }
      }
    } finally {
      await browser.close();
    }
  });
}
