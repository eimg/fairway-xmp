// Run with npm run site:dev serving the documentation in another terminal.
import assert from 'node:assert/strict';
import test from 'node:test';
import { chromium, webkit, expect } from '@playwright/test';

const base = process.env.SITE_BASE_URL || 'http://127.0.0.1:4173/fairway-xmp/';

// Tap a sticky control's visible box without Playwright scrolling its flow box.
async function tap(page, locator) {
  const bounds = await locator.boundingBox();
  assert(bounds);
  await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
}

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
          for (const chapter of ['agent-skill', 'terminal', 'sites', 'settings', 'cli', 'updates']) {
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
          if (width <= 760) await tap(page, page.locator('[data-guide-menu-trigger]'));
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
          await expect(page.locator('[data-guide-search]')).toBeHidden();
          for (const link of await page.locator('[data-guide-search-link]').all()) await expect(link).toBeHidden();
          await expect(page.locator('.mobile-contents')).toBeVisible({ visible: width < 760 });
          await page.locator('.common-tasks a[href="#sites"]').click();
          assert.equal(new URL(page.url()).hash, '#sites');
          await expect(page.locator('#sites .guide-steps > li')).toHaveCount(5);
          const help = page.locator('#sites .guide-troubleshooting details').first();
          await help.locator('summary').click();
          await expect(help.locator('p')).toBeVisible();
          await page.locator('#sites .guide-related a[href="#mysql"]').click();
          await expect(page.locator('#mysql pre')).toContainText('DB_HOST=127.0.0.1');
          await page.goto(`${base}${route}#cli`);
          const cli = page.locator('#cli');
          await expect(cli.locator('.guide-agent')).toBeVisible();
          await expect(cli.locator('.guide-agent pre')).toContainText('fairway capabilities --json');
          await expect(cli.locator('.guide-agent pre')).toContainText('fairway status --json');
          await expect(cli.locator('.guide-agent a')).toHaveAttribute('href', 'https://github.com/eimg/fairway-xmp/tree/main/skills/fairway-xmp');
          assert(await cli.evaluate(section => section.querySelector('.guide-agent').compareDocumentPosition(section.querySelector('.guide-steps')) & Node.DOCUMENT_POSITION_FOLLOWING), `${route}: agent instructions precede the procedure`);
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

for (const [name, engine] of Object.entries({ chromium, webkit })) {
  test(`${name}: search modal finds guides, supports keyboards and preserves reading position`, async () => {
    const browser = await engine.launch();
    try {
      const page = await browser.newPage({ reducedMotion: 'reduce' });
      for (const width of [1280, 390, 320]) {
        await page.setViewportSize({ width, height: 900 });
        for (const route of ['', 'mm/']) {
          await page.goto(`${base}${route}#composer`);
          const panel = page.locator('#guide-search');
          const query = page.locator('#guide-query');
          const results = page.locator('#guide-search-results');
          const shortcut = page.locator(`${width > 760 ? '.docs-sidebar' : '.header'} [data-guide-search-link]`);
          await expect(panel).toBeHidden();
          await expect(page.locator('[data-guide-search-link]:visible')).toHaveCount(1);
          const originalPosition = await page.evaluate(() => scrollY);
          await tap(page, shortcut);
          await expect(query).toBeFocused();
          await query.fill('SKILL.md');
          await expect(results.locator('a[href="#agent-skill"]')).toBeVisible();
          await page.keyboard.press('Escape');
          await expect(panel).toBeHidden();
          await expect(shortcut).toBeFocused();
          assert.equal(await page.evaluate(() => scrollY), originalPosition);
          assert.equal(new URL(page.url()).hash, '#composer');
          assert.equal(await page.evaluate(() => document.body.style.overflow), '');
          await tap(page, shortcut);
          await expect(query).toHaveValue('SKILL.md');
          await query.press('ArrowDown');
          await expect(results.locator('a').first()).toBeFocused();
          await page.keyboard.press('Enter');
          await expect(panel).toBeHidden();
          assert.equal(new URL(page.url()).hash, '#agent-skill');
          await expect(page.locator('#agent-skill h2')).toBeFocused();
          await tap(page, shortcut);
          await query.fill('approval_required');
          await expect(results.locator('a[href="#cli"]')).toBeVisible();
          await expect(results).toContainText('approval_required');
          await query.fill('19000');
          await expect(results.locator('a[href="#default-ports"]')).toBeVisible();
          const firstTarget = await results.locator('a').first().getAttribute('href');
          await query.press('Enter');
          await expect(panel).toBeHidden();
          assert.equal(new URL(page.url()).hash, firstTarget);
          await expect(page.locator(`${firstTarget} h2`)).toBeFocused();
          await tap(page, shortcut);
          if (route) {
            await query.fill('ထည့်သွင်း');
            await expect(results.locator('a[href="#agent-skill"]')).toBeVisible();
          }
          await query.fill('<script>no-match</script>');
          await expect(results.locator('li')).toHaveCount(0);
          await expect(page.locator('.guide-search-status')).toHaveText(route ? 'လမ်းညွှန် မတွေ့ပါ။ အခြား စကားလုံး သို့မဟုတ် Command ဖြင့် ရှာပါ။' : 'No matching guides. Try another word or command.');
          await panel.locator('button[type="reset"]').click();
          await expect(query).toBeFocused();
          await expect(query).toHaveValue('');
          await expect(results).toBeHidden();
          await expect(page.locator('.guide-search-status')).not.toBeEmpty();
          const nextKey = process.platform === 'darwin' && name === 'webkit' ? 'Alt+Tab' : 'Tab';
          await page.keyboard.press(nextKey);
          assert(await panel.evaluate(dialog => dialog.contains(document.activeElement)), 'search keeps keyboard focus inside');
          await panel.locator('[data-close-search]').click();
          await expect(shortcut).toBeFocused();
          await tap(page, shortcut);
          await page.mouse.click(width - 3, 898);
          await expect(panel).toBeHidden();
          await expect(shortcut).toBeFocused();
          assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${route}: search fits ${width}px`);
        }
      }
    } finally { await browser.close(); }
  });
}

for (const [name, engine] of Object.entries({ chromium, webkit })) {
  test(`${name}: mobile sidebar stays accessible while reading and restores navigation focus`, async () => {
    const browser = await engine.launch();
    try {
      const page = await browser.newPage({ reducedMotion: 'reduce' });
      const drawer = page.locator('#guide-menu');
      const trigger = page.locator('[data-guide-menu-trigger]');
      // Locator.click scrolls sticky elements' flow boxes into view; use the
      // visible hit target to exercise a real tap without moving the chapter.
      const clickTrigger = async () => {
        const bounds = await trigger.boundingBox();
        assert(bounds);
        await page.mouse.click(bounds.x + bounds.width / 2, bounds.y + bounds.height / 2);
      };
      for (const width of [760, 390, 320]) {
        await page.setViewportSize({ width, height: 900 });
        for (const route of ['', 'mm/']) {
          await page.goto(`${base}${route}#composer`);
          await expect(page.locator('.header nav')).toHaveCount(0);
          await expect(page.locator('.header .header-preferences')).toHaveCount(0);
          await expect(page.locator('[data-guide-search-link]:visible')).toHaveCount(1);
          const search = page.locator('.header [data-guide-search-link]');
          await expect(search).toHaveText('');
          await expect(search).toHaveAccessibleName(route ? 'လမ်းညွှန် ရှာရန်' : 'Search guides');
          await expect(page.locator('.mobile-contents')).toBeHidden();
          await expect(trigger).toBeInViewport();
          assert((await page.locator('.header').boundingBox()).height <= 68, 'mobile header fits one compact row');
          await expect.poll(() => page.locator('.header').evaluate(bar => Math.round(bar.getBoundingClientRect().top))).toBe(0);
          const readingPosition = await page.evaluate(() => scrollY);
          await clickTrigger();
          await expect(drawer).toBeVisible();
          await expect(drawer.locator('.languages')).toBeVisible();
          await expect(drawer.locator('[data-theme-toggle]')).toBeVisible();
          await expect(drawer.locator('[data-open-downloads]')).toBeVisible();
          const bounds = await drawer.boundingBox();
          assert(bounds && Math.abs(bounds.x) < 1 && Math.abs(bounds.y) < 1, 'drawer attaches to the left viewport edge');
          assert(bounds.width <= width * 0.85 + 1, 'drawer leaves a visible backdrop beside it');
          assert.equal(Math.round(bounds.height), 900, 'drawer fills the viewport height');
          await expect(trigger).toHaveAttribute('aria-expanded', 'true');
          await expect(drawer.locator('a[href="#composer"]')).toHaveAttribute('aria-current', 'location');
          assert.equal(await page.evaluate(() => document.body.style.overflow), 'hidden');
          const nextKey = process.platform === 'darwin' && name === 'webkit' ? 'Alt+Tab' : 'Tab';
          for (let step = 0; step < 3; step++) {
            await page.keyboard.press(nextKey);
            assert(await drawer.evaluate(menu => menu.contains(document.activeElement)), 'focus stays inside the modal menu');
          }
          await page.keyboard.press('Escape');
          await expect(drawer).not.toBeVisible();
          await expect(trigger).toBeFocused();
          await expect(trigger).toHaveAttribute('aria-expanded', 'false');
          assert.equal(await page.evaluate(() => document.body.style.overflow), '');
          assert.equal(await page.evaluate(() => scrollY), readingPosition);
          await clickTrigger();
          await drawer.locator('[data-guide-menu-close]').click();
          await expect(trigger).toBeFocused();
          await clickTrigger();
          await page.mouse.click(width - 4, 300);
          await expect(drawer).not.toBeVisible();
          await clickTrigger();
          await drawer.locator('a[href="#mysql"]').click();
          await expect(drawer).not.toBeVisible();
          assert.equal(new URL(page.url()).hash, '#mysql');
          await expect(page.locator('#mysql h2')).toBeFocused();
          assert(await page.locator('#mysql').evaluate(section => section.getBoundingClientRect().top >= 60), 'sticky controls do not cover the chapter');
          await tap(page, page.locator('.header [data-guide-search-link]'));
          await expect(page.locator('#guide-query')).toBeFocused();
          await page.keyboard.press('Escape');
          assert(await page.evaluate(() => document.documentElement.scrollWidth <= innerWidth), `${route}: drawer and toolbar fit ${width}px`);
        }
      }
      await clickTrigger();
      await page.setViewportSize({ width: 1280, height: 900 });
      await expect(drawer).not.toBeVisible();
      await expect(page.locator('[data-guide-menu-trigger]')).toBeHidden();
      await expect(page.locator('.header nav')).toBeVisible();
      await expect(page.locator('.header .header-preferences')).toBeVisible();
      await expect(page.locator('.docs-sidebar')).toBeVisible();
      assert.equal(await page.evaluate(() => document.body.style.overflow), '');
    } finally {
      await browser.close();
    }
  });
}

for (const [name, engine] of Object.entries({ chromium, webkit })) {
  test(`${name}: drawer downloads and responsive search preserve modal focus and scroll locks`, async () => {
    const browser = await engine.launch();
    try {
      const page = await browser.newPage({ viewport: { width: 390, height: 900 }, reducedMotion: 'reduce' });
      await page.goto(`${base}#composer`);
      const drawer = page.locator('#guide-menu');
      const downloads = page.locator('#download-dialog');
      const trigger = page.locator('[data-open-downloads]');
      await tap(page, page.locator('[data-guide-menu-trigger]'));
      await trigger.click();
      await expect(drawer).toBeVisible();
      await expect(downloads).toBeVisible();
      await page.keyboard.press('Escape');
      await expect(downloads).toBeHidden();
      await expect(trigger).toBeFocused();
      assert.equal(await page.evaluate(() => document.body.style.overflow), 'hidden');
      await page.keyboard.press('Escape');
      await expect(drawer).toBeHidden();
      assert.equal(await page.evaluate(() => document.body.style.overflow), '');
      await tap(page, page.locator('[data-guide-menu-trigger]'));
      await trigger.click();
      await page.setViewportSize({ width: 1280, height: 900 });
      await expect(drawer).toBeHidden();
      await expect(downloads).toBeVisible();
      assert.equal(await page.evaluate(() => document.body.style.overflow), 'hidden');
      await page.keyboard.press('Escape');
      await expect(trigger).toBeFocused();
      assert.equal(await page.evaluate(() => document.body.style.overflow), '');
      await tap(page, page.locator('.docs-sidebar [data-guide-search-link]'));
      await page.setViewportSize({ width: 320, height: 900 });
      await expect(page.locator('#guide-search')).toBeVisible();
      assert(await page.locator('#guide-search').evaluate(dialog => dialog.getBoundingClientRect().right <= innerWidth));
      await page.keyboard.press('Escape');
      await expect(page.locator('.header [data-guide-search-link]')).toBeFocused();
      assert.equal(await page.evaluate(() => document.body.style.overflow), '');
    } finally { await browser.close(); }
  });
}
