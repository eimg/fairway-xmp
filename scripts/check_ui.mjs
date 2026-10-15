import { createRequire } from 'node:module';
import { mkdir } from 'node:fs/promises';
import assert from 'node:assert/strict';

const require = createRequire(import.meta.url);
const playwright = process.env.FAIRWAY_PLAYWRIGHT_MODULE_DIR
  ? require(`${process.env.FAIRWAY_PLAYWRIGHT_MODULE_DIR}/playwright`)
  : require('playwright');
const browser = await playwright.chromium.launch({ headless: true });
const page = await browser.newPage({ viewport: { width: 1080, height: 810 }, deviceScaleFactor: 1 });
const errors = [];
page.on('pageerror', error => errors.push(error.message));
await mkdir('artifacts', { recursive: true });
try {
  await page.goto('http://127.0.0.1:1420');
  await page.getByRole('heading', { name: 'Ready when you are.' }).waitFor();
  assert(await page.getByRole('button', { name: 'Start all', exact: true }).isDisabled());
  await page.screenshot({ path: 'artifacts/control-panel.png', fullPage: true });
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  assert.equal(await page.getByRole('spinbutton', { name: /^Website/ }).inputValue(), '8800');
  await page.getByRole('button', { name: 'Updates', exact: true }).click();
  await page.getByRole('heading', { name: 'A dependable environment.' }).waitFor();
  await page.getByRole('button', { name: /Getting started/ }).click();
  await page.getByRole('dialog', { name: 'Your first PHP project' }).waitFor();
  await page.keyboard.press('Escape');
  assert.equal(await page.getByRole('dialog').count(), 0);
  await page.setViewportSize({ width: 820, height: 700 });
  assert(await page.evaluate(() => document.documentElement.scrollWidth <= window.innerWidth));
  assert.deepEqual(errors, []);
  console.log('PASS: preview controls, settings, updates, help dialog, responsive layout, and browser errors.');
} finally { await browser.close(); }
