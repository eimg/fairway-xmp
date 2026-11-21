import { expect, test, type Page } from '@playwright/test';
import { readFileSync } from 'node:fs';
import type { Snapshot } from '../../src/types';

const externalRequests = new WeakMap<Page, string[]>();
const csp = JSON.parse(readFileSync(new URL('../../src-tauri/tauri.conf.json', import.meta.url), 'utf8')).app.security.csp as string;

test.beforeEach(async ({ page }) => {
  await page.goto('/');
  await page.getByLabel('Scenario', { exact: true }).selectOption('Running');
  const initial = await page.evaluate(async () => {
    const path = '/src/ui-test/backend.ts';
    return (await import(path)).invoke('snapshot') as Promise<Snapshot>;
  });
  await page.addInitScript((snapshot: Snapshot) => {
    const image = 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7';
    const email = {
      ID: 'boundary', Read: false, From: { Name: 'Local test', Address: 'local@shop.test' }, To: [], Cc: [],
      Subject: 'Boundary email', Date: new Date().toISOString(), Created: new Date().toISOString(),
      Snippet: 'Readable email', Attachments: [], InlineImages: { 'sample@fairway.test': image }, Text: 'Readable text version',
      HTML: `<html><head><meta http-equiv="refresh" content="0;url=https://blocked.invalid/refresh"></head><body>
        <p>Readable email</p><img alt="CID image" src="cid:sample%40fairway.test">
        <script>parent.postMessage({type:'fairway-mail-preview',action:'open',url:'https://blocked.invalid/script'},'*')</script>
        <img alt="Remote image" src="https://blocked.invalid/tracker" onload="parent.postMessage({type:'fairway-mail-preview',action:'open',url:'https://blocked.invalid/automatic'},'*')">
        <iframe src="https://blocked.invalid/frame"></iframe><base href="https://blocked.invalid/">
        <form action="https://blocked.invalid/submit" target="_top"><button>Submit email form</button></form>
        <a href="https://blocked.invalid/navigate" target="_top">Email link</a>
        <a href="file:///tmp/demo">File link</a></body></html>`,
    };
    const callbacks = new Map<number, (event: { payload: unknown }) => void>();
    let next = 0;
    window.__TAURI_INTERNALS__ = {
      transformCallback(callback) { callbacks.set(++next, callback); return next; },
      unregisterCallback(id) { callbacks.delete(id); },
      async invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
        if (command === 'plugin:event|listen') return ++next as T;
        if (command === 'plugin:event|unlisten') return undefined as T;
        if (command === 'snapshot') return structuredClone(snapshot) as T;
        if (command === 'startup_status') return { blocking: false, errors: [], dataDir: snapshot.dataDir } as T;
        if (command === 'startup_services') return { nginx: false, mysql: false, mail: false } as T;
        if (command === 'login_item_status') return { registered: false, state: 'notRegistered' } as T;
        if (command === 'open_mail_link') {
          const links: string[] = JSON.parse(document.documentElement.dataset.mailLinks ?? '[]');
          document.documentElement.dataset.mailLinks = JSON.stringify([...links, args.url]);
          return undefined as T;
        }
        if (command === 'load_mail_image') {
          document.documentElement.dataset.imageRequest = String(args.url);
          return image as T;
        }
        if (command === 'mail_inbox') {
          const request = args.request as { action: string };
          if (request.action === 'list') return { messages: [{ ...email, Attachments: 0 }], messages_count: 1, messages_unread: 1 } as T;
          return email as T;
        }
        throw new Error(`Unexpected command: ${command}`);
      },
    };
  }, initial);
  // Apply the actual app policy to production assets and inherited srcdoc frames.
  await page.route('http://127.0.0.1:1422/', async route => {
    const response = await route.fetch();
    await route.fulfill({ response, headers: { ...response.headers(), 'content-security-policy': csp } });
  });
  externalRequests.set(page, []);
  await page.route('**/blocked.invalid/**', route => {
    externalRequests.get(page)!.push(route.request().url());
    return route.abort();
  });
  await page.goto('http://127.0.0.1:1422/');
});

test('production CSP isolates untrusted mail and routes deliberate web links through native IPC', async ({ page }) => {
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await page.getByRole('list', { name: 'Emails', exact: true }).getByRole('button', { name: /Boundary email/ }).click();
  const frame = page.frameLocator('iframe[title="Email content"]');
  await expect(frame.getByText('Readable email', { exact: true })).toBeVisible();
  await expect(page.locator('iframe[title="Email content"]')).toHaveAttribute('sandbox', 'allow-scripts');
  await expect(frame.locator('meta[http-equiv=refresh], base')).toHaveCount(0);
  // Only the trusted, CSP-hashed interaction bridge remains executable.
  await expect(frame.locator('script')).toHaveCount(1);
  await expect.poll(() => frame.getByAltText('CID image').evaluate(img => (img as HTMLImageElement).naturalWidth)).toBe(1);
  await expect(frame.getByAltText('Remote image')).toHaveJSProperty('naturalWidth', 1);
  await expect(page.locator('html')).toHaveAttribute('data-image-request', 'https://blocked.invalid/tracker');
  // CSP blocks email event handlers and nested frame loads even when their
  // inert markup is retained. Neither may trigger an external open or request.
  expect(await page.locator('html').getAttribute('data-mail-links')).toBeNull();
  await page.evaluate(() => new Promise<void>(resolve => {
    window.postMessage({ type: 'fairway-mail-preview', action: 'open', url: 'https://blocked.invalid/forged' }, '*');
    setTimeout(resolve, 0);
  }));
  expect(await page.locator('html').getAttribute('data-mail-links')).toBeNull();
  await frame.getByRole('link', { name: 'Email link', exact: true }).click();
  const links = JSON.stringify(['https://blocked.invalid/navigate']);
  await expect(page.locator('html')).toHaveAttribute('data-mail-links', links);
  await frame.getByRole('link', { name: 'File link', exact: true }).click();
  await frame.getByRole('button', { name: 'Submit email form', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-mail-links', links);
  await expect.poll(() => page.evaluate(() => location.href)).toBe('http://127.0.0.1:1422/');
  expect(externalRequests.get(page)).toEqual([]);
});
