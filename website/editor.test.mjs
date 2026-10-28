// Saves only into a temporary copy of the website, never the working translations.
import assert from 'node:assert/strict';
import test from 'node:test';
import { cp, mkdir, mkdtemp, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { pathToFileURL } from 'node:url';
import { chromium, webkit, expect } from '@playwright/test';

for (const [name, engine] of Object.entries({ chromium, webkit })) {
  test(`${name}: edit, recover a browser draft, save Burmese, preview and reject stale writes`, { timeout: 30000 }, async () => {
    const scratch = await mkdtemp(join(tmpdir(), 'fairway-translation-editor-'));
    let server, browser;
    try {
      const site = join(scratch, 'website');
      await cp(new URL('./', import.meta.url), site, { recursive: true, filter: source => !/\/(dist|\.translation-backups)(\/|$)/.test(source) });
      await mkdir(join(scratch, 'src-tauri/icons'), { recursive: true });
      await cp(new URL('../src-tauri/icons/128x128.png', import.meta.url), join(scratch, 'src-tauri/icons/128x128.png'));
      const baseline = await Promise.all(['content.mjs', 'guides.mjs'].map(file => readFile(join(site, file), 'utf8')));
      const before = (await import(pathToFileURL(join(site, 'content.mjs')))).copy.en;
      server = spawn(process.execPath, [join(site, 'edit-translations.mjs')], { env: { ...process.env, SITE_EDITOR_PORT: '0' }, stdio: ['ignore', 'pipe', 'pipe'] });
      const base = await new Promise((accept, reject) => {
        let output = '';
        server.stdout.on('data', chunk => {
          output += chunk;
          const match = output.match(/Burmese editor: (http:\/\/127\.0\.0\.1:\d+\/)/);
          if (match) accept(match[1]);
        });
        server.stderr.on('data', chunk => { output += chunk; });
        server.once('error', reject);
        server.once('exit', code => reject(new Error(`Editor exited ${code}: ${output}`)));
      });
      const state = await (await fetch(base + 'api/catalog')).json();
      browser = await engine.launch();
      const context = await browser.newContext();
      const page = await context.newPage();
      const errors = [];
      page.on('pageerror', error => errors.push(error.message));
      await page.goto(base);
      await expect(page.locator('#status')).toContainText('Ready.');
      await page.locator('#section').selectOption('nginx');
      const row = page.locator('[data-field="nginx.step.0"]');
      await expect(row.locator('.reference')).toContainText('Environment');
      assert.equal(await row.locator('.translation code [contenteditable], .translation strong [contenteditable]').count(), 0);
      const editable = row.locator('[data-segment]').first();
      await editable.fill('မြန်မာစာ တည်းဖြတ်မှု ');
      await expect(page.locator('#status')).toContainText('1 unsaved');
      // Reloading retains the in-browser draft for the same source revision.
      page.once('dialog', dialog => dialog.accept());
      await page.reload();
      await expect(page.locator('#status')).toContainText('Restored 1');
      await page.locator('#section').selectOption('nginx');
      await expect(row.locator('[data-segment]').first()).toContainText('မြန်မာစာ တည်းဖြတ်မှု');
      await page.locator('#save').click();
      await expect(page.locator('#status')).toContainText('Saved 1');
      const [{ copy }, { guides }] = await Promise.all(['content.mjs', 'guides.mjs'].map(file => import(pathToFileURL(join(site, file)) + '?saved')));
      assert(guides[0].steps[1][0].startsWith('မြန်မာစာ တည်းဖြတ်မှု '));
      assert(guides[0].steps[1][0].includes('<code>fairway/www</code>'));
      assert.deepEqual(copy.en, before);
      assert.equal((await readdir(join(site, '.translation-backups'))).length, 1);
      const preview = await page.context().newPage();
      await preview.goto(base + 'preview/mm/#nginx');
      await expect(preview.locator('#nginx .guide-steps')).toContainText('မြန်မာစာ တည်းဖြတ်မှု');
      const en = await (await fetch(base + 'preview/')).text();
      assert(!en.includes('မြန်မာစာ တည်းဖြတ်မှု'));
      assert((await fetch(base + 'preview/styles.css')).ok);
      const unauthorized = await fetch(base + 'api/save', { method: 'POST', headers: { 'Content-Type': 'application/json', Origin: 'https://outside.example' }, body: '{}' });
      assert.equal(unauthorized.status, 403);
      const stale = await fetch(base + 'api/save', { method: 'POST', headers: { 'Content-Type': 'application/json', Origin: base.slice(0, -1), 'X-Editor-Token': state.token }, body: JSON.stringify({ revision: state.revision, changes: [] }) });
      assert.equal(stale.status, 400);
      await expect(page.locator('#save')).toBeDisabled();
      assert.deepEqual(errors, []);
      // The copied source has changed; originals were never edited by this test.
      assert.notEqual(await readFile(join(site, 'guides.mjs'), 'utf8'), baseline[1]);
    } finally {
      await browser?.close();
      if (server && server.exitCode === null) { server.kill(); await once(server, 'exit'); }
      await rm(scratch, { recursive: true, force: true });
    }
  });
}
