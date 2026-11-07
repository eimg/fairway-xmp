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
    const data = structuredClone(snapshot); data.httpsTrustUpdate = true;
    const callbacks = new Map<number, (event: { payload: unknown }) => void>(); let next = 0;
    let cancelled = true;
    window.addEventListener('test-approve-trust', () => { cancelled = false; });
    const email = { ID: 'boundary', Read: false, From: { Name: 'Local test', Address: 'local@shop.test' }, To: [], Cc: [], Subject: 'Boundary email', Date: new Date().toISOString(), Created: new Date().toISOString(), Snippet: 'Readable email', Attachments: [], InlineImages: { 'sample@fairway.test': 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7' }, Text: 'Readable text version', HTML: '<html><head><meta http-equiv="refresh" content="0;url=https://blocked.invalid/refresh"></head><body style="background-color:rgb(240,245,241)"><p id="readable" style="color:rgb(20,80,30)">Readable email</p><img alt="Embedded image" onload="parent.postMessage({type:&quot;fairway-mail-preview&quot;,action:&quot;open&quot;,url:&quot;https://blocked.invalid/automatic&quot;},&quot;*&quot;)" src="data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7"><img alt="CID image" src="cid:sample%40fairway.test"><script>parent.document.documentElement.dataset.emailScript="ran"</script><base href="https://blocked.invalid/"><object data="/?blocked-object"></object><iframe src="https://blocked.invalid/frame"></iframe><img src="https://blocked.invalid/tracker"><form action="https://blocked.invalid/submit" target="_top"><button>Submit email form</button></form><a href="https://blocked.invalid/navigate" target="_top"><span>Email link</span></a><a href="http://project.test:8800/reset?token=sample" target="_blank">Reset password</a><a href="javascript:void(0)">Script link</a><a href="file:///tmp/demo">File link</a><a href="data:text/plain,test">Data link</a><a href="/relative">Relative link</a></body></html>' };
    email.HTML += '<style>@import url(https://blocked.invalid/style);@font-face{font-family:remote;src:url(https://blocked.invalid/font)}.background-image{background-image:url(https://blocked.invalid/background);font-family:remote}</style><p class="background-image">Blocked background</p><img alt="Second remote image" src="http://project.test:8800/logo.png"><picture><source srcset="https://blocked.invalid/picture"><img alt="Responsive remote image" srcset="https://blocked.invalid/responsive 1x, https://blocked.invalid/retina 2x"></picture><img alt="Invalid credentials" src="https://user:pass@blocked.invalid/credentials"><span data-fairway-image="999"><button>Forged image control</button></span>';
    const otherEmail = { ...email, ID: 'other', Subject: 'Other email' };
    let imageDelay = 0;
    let failImage = false;
    window.addEventListener('test-mail-image-options', event => {
      const options = (event as CustomEvent<{ delay: number; fail: boolean }>).detail;
      imageDelay = options.delay; failImage = options.fail;
    });
    document.addEventListener('securitypolicyviolation', event => {
      const old: string[] = JSON.parse(document.documentElement.dataset.violations ?? '[]');
      document.documentElement.dataset.violations = JSON.stringify([...old, event.effectiveDirective]);
    });
    window.__TAURI_INTERNALS__ = {
      transformCallback(callback) { callbacks.set(++next, callback); return next; }, unregisterCallback(id) { callbacks.delete(id); },
      async invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
        if (command === 'plugin:event|listen') return ++next as T;
        if (command === 'plugin:event|unlisten') return undefined as T;
        if (command === 'snapshot') return structuredClone(data) as T;
        if (command === 'startup_status') return { blocking: false, errors: [], dataDir: data.dataDir } as T;
        if (command === 'startup_services') return { nginx: false, mysql: false, mail: false } as T;
        if (command === 'login_item_status') return { registered: false, state: 'notRegistered' } as T;
        if (command === 'set_project_https') {
          document.documentElement.dataset.httpsRequest = JSON.stringify(args);
          if (cancelled) throw new Error('Certificate approval was cancelled. Existing HTTPS sites were kept.');
          data.httpsTrustUpdate = false; return structuredClone(data) as T;
        }
        if (command === 'save_project') { document.documentElement.dataset.saved = 'yes'; return structuredClone(data) as T; }
        if (command === 'open_target') { document.documentElement.dataset.opened = String(args.target); return undefined as T; }
        if (command === 'open_mail_link') {
          const opened: string[] = JSON.parse(document.documentElement.dataset.mailLinks ?? '[]');
          document.documentElement.dataset.mailLinks = JSON.stringify([...opened, args.url]);
          return undefined as T;
        }
        if (command === 'load_mail_image') {
          const urls: string[] = JSON.parse(document.documentElement.dataset.imageRequests ?? '[]');
          document.documentElement.dataset.imageRequests = JSON.stringify([...urls, args.url]);
          const fail = failImage;
          if (imageDelay) await new Promise(resolve => setTimeout(resolve, imageDelay));
          if (fail) throw new Error('Image server returned HTTP 404.');
          return 'data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7' as T;
        }
        if (command === 'mail_inbox') {
          const request = args.request as { action: string; id?: string };
          if (request.action === 'list') return { messages: [{ ...email, Attachments: 0 }, { ...otherEmail, Attachments: 0 }], messages_count: 2, messages_unread: 2 } as T;
          return (request.id === 'other' ? otherEmail : email) as T;
        }
        throw new Error(`Unexpected command: ${command}`);
      },
    };
  }, initial);
  // Exercise the exact configured production CSP, including inherited srcdoc
  // policy. Debug-protocol evaluation supplies only the controlled native bridge.
  await page.route('http://127.0.0.1:1422/', async route => {
    const response = await route.fetch();
    await route.fulfill({ response, headers: { ...response.headers(), 'content-security-policy': csp } });
  });
  externalRequests.set(page, []);
  await page.route('**/blocked.invalid/**', route => { externalRequests.get(page)!.push(route.request().url()); return route.abort(); });
  await page.goto('http://127.0.0.1:1422/');
  await expect(page.getByRole('button', { name: 'Stop all', exact: true })).toBeVisible();
});

test('certificate update cancellation keeps HTTPS selected and retry retires the pending update', async ({ page }) => {
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
  const update = page.getByRole('button', { name: 'Update certificate trust', exact: true });
  await expect(update).toBeVisible();
  const enabled = page.getByRole('switch', { name: 'HTTPS for My Shop', exact: true });
  await expect(enabled).toBeChecked();
  await update.click();
  await expect(page.getByRole('alert')).toContainText('approval was cancelled');
  await expect(enabled).toBeChecked(); await expect(update).toBeEnabled();
  expect(JSON.parse((await page.locator('html').getAttribute('data-https-request'))!)).toEqual({ domain: 'shop', enabled: true });
  await page.evaluate(() => window.dispatchEvent(new Event('test-approve-trust')));
  await update.click();
  await expect(update).toHaveCount(0); await expect(enabled).toBeChecked();
});

test('production CSP allows mail rendering, native external opens and React dialog submission', async ({ page }) => {
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await page.getByRole('list', { name: 'Emails', exact: true }).getByRole('button', { name: /Boundary email/ }).click();
  const frame = page.frameLocator('iframe[title="Email content"]');
  await expect(frame.locator('#readable')).toHaveText('Readable email');
  await expect(frame.locator('body')).toHaveCSS('background-color', 'rgb(240, 245, 241)');
  await expect(frame.locator('meta[http-equiv=refresh]')).toHaveCount(0);
  await expect(frame.locator('#readable')).toHaveCSS('color', 'rgb(20, 80, 30)');
  await expect(page.locator('iframe[title="Email content"]')).toHaveAttribute('sandbox', 'allow-scripts');
  expect(await page.locator('html').getAttribute('data-email-script')).toBeNull();
  await expect.poll(() => frame.getByAltText('Embedded image').evaluate(img => (img as HTMLImageElement).naturalWidth)).toBe(1);
  await expect.poll(() => frame.getByAltText('CID image').evaluate(img => (img as HTMLImageElement).naturalWidth)).toBe(1);
  await expect(frame.getByAltText('CID image')).toHaveAttribute('src', /^data:image\/gif;base64,/);
  expect(externalRequests.get(page)).toEqual([]);
  await expect(frame.locator('script')).toHaveCount(1);
  expect(await page.locator('html').getAttribute('data-mail-links')).toBeNull();
  await page.evaluate(() => new Promise<void>(resolve => { window.postMessage({ type: 'fairway-mail-preview', action: 'open', url: 'https://blocked.invalid/forged' }, '*'); setTimeout(resolve, 0); }));
  expect(await page.locator('html').getAttribute('data-mail-links')).toBeNull();
  const emailLink = frame.getByRole('link', { name: 'Email link', exact: true });
  await emailLink.locator('span').click();
  await frame.getByRole('link', { name: 'Reset password', exact: true }).click();
  await emailLink.click({ button: 'middle' });
  await emailLink.focus();
  await page.keyboard.press('Enter');
  const expectedLinks = ['https://blocked.invalid/navigate', 'http://project.test:8800/reset?token=sample', 'https://blocked.invalid/navigate', 'https://blocked.invalid/navigate'];
  await expect(page.locator('html')).toHaveAttribute('data-mail-links', JSON.stringify(expectedLinks));
  for (const name of ['Script link', 'File link', 'Data link', 'Relative link']) {
    await frame.getByRole('link', { name, exact: true }).click();
  }
  await frame.getByRole('button', { name: 'Submit email form', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-mail-links', JSON.stringify(expectedLinks));
  expect(externalRequests.get(page)).toEqual([]);
  await expect(page).toHaveURL('http://127.0.0.1:1422/');
  await page.getByRole('tab', { name: 'Text', exact: true }).click();
  await expect(page.locator('.mail-text')).toHaveText('Readable text version');
  await page.getByRole('button', { name: 'Open in browser', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-opened', 'mail');
  await page.getByRole('button', { name: 'Manage Sites', exact: true }).click();
  await page.getByRole('button', { name: 'Actions for My Shop', exact: true }).click();
  await page.getByRole('menuitem', { name: 'Edit site', exact: true }).click();
  const dialog = page.getByRole('dialog', { name: 'Edit site', exact: true });
  await dialog.getByLabel('Site name', { exact: true }).fill('Updated title');
  await dialog.getByRole('button', { name: 'Save changes', exact: true }).click();
  await expect(page.locator('html')).toHaveAttribute('data-saved', 'yes');
  await expect(dialog).toHaveCount(0);
});

test('production CSP blocks object loads, base changes and unhandled form navigation', async ({ page }) => {
  const before = page.url();
  await page.evaluate(() => {
    const base = document.createElement('base'); base.href = 'https://blocked.invalid/'; document.head.append(base);
    const object = document.createElement('object'); object.data = '/?blocked-object'; document.body.append(object);
  });
  await expect.poll(() => page.evaluate(() => JSON.parse(document.documentElement.dataset.violations ?? '[]'))).toEqual(expect.arrayContaining(['base-uri', 'object-src']));
  await page.evaluate(() => { const form = document.createElement('form'); form.action = 'https://blocked.invalid/submit'; document.body.append(form); form.submit(); });
  // Chromium reports a blank provisional URL for a CSP-blocked submission;
  // check the still-active document and UI instead of that provisional frame.
  await expect.poll(() => page.evaluate(() => location.href)).toBe(before);
  expect(externalRequests.get(page)).toEqual([]);
  expect(await page.evaluate(() => document.baseURI)).toBe(before);
  await expect.poll(() => page.evaluate(() => JSON.parse(document.documentElement.dataset.violations ?? '[]'))).toEqual(expect.arrayContaining(['base-uri', 'object-src', 'form-action']));
  await page.evaluate(() => { [...document.querySelectorAll('button')].find(button => button.textContent?.trim() === 'Manage Sites')!.click(); });
  await expect.poll(() => page.evaluate(() => [...document.querySelectorAll('h1')].find(heading => heading.getClientRects().length > 0)?.textContent)).toBe('Your sites');
});

test('remote images load automatically under production CSP and survive HTML/Text switches', async ({ page }) => {
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  const emails = page.getByRole('list', { name: 'Emails', exact: true });
  await emails.getByRole('button', { name: /Boundary email/ }).click();
  const frame = page.frameLocator('iframe[title="Email content"]');
  const expected = ['https://blocked.invalid/tracker', 'http://project.test:8800/logo.png', 'https://blocked.invalid/responsive'];
  await expect(page.locator('html')).toHaveAttribute('data-image-requests', JSON.stringify(expected));
  await expect(frame.locator('img[data-fairway-loaded]')).toHaveCount(3);
  await expect(frame.getByAltText('Second remote image')).toHaveJSProperty('naturalWidth', 1);
  await expect(page.getByRole('checkbox', { name: 'Load remote images', exact: true })).toHaveCount(0);
  await expect(frame.getByRole('button', { name: /^(Load|Retry) image$/ })).toHaveCount(0);
  await expect(frame.locator('[data-fairway-image="999"]')).toHaveCount(0);
  // A forged request from outside the current frame cannot retry an image.
  await page.evaluate(() => window.postMessage({ type: 'fairway-mail-preview', action: 'image', id: '0' }, '*'));
  await page.getByRole('tab', { name: 'Text', exact: true }).click();
  await expect(page.locator('.mail-text')).toHaveText('Readable text version');
  await page.getByRole('tab', { name: 'HTML', exact: true }).click();
  await expect(frame.locator('img[data-fairway-loaded]')).toHaveCount(3);
  await expect(page.locator('html')).toHaveAttribute('data-image-requests', JSON.stringify(expected));
  expect(externalRequests.get(page)).toEqual([]);
  expect(await page.locator('html').getAttribute('data-email-script')).toBeNull();
  expect(await page.locator('html').getAttribute('data-mail-links')).toBeNull();
  await emails.getByRole('button', { name: /Other email/ }).click();
  await expect(page.locator('html')).toHaveAttribute('data-image-requests', JSON.stringify([...expected, ...expected]));
  await expect(frame.locator('img[data-fairway-loaded]')).toHaveCount(3);
});

test('failed automatic downloads offer keyboard-accessible retries only after failure', async ({ page }) => {
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('test-mail-image-options', { detail: { delay: 200, fail: true } })));
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  await page.getByRole('list', { name: 'Emails', exact: true }).getByRole('button', { name: /Boundary email/ }).click();
  const frame = page.frameLocator('iframe[title="Email content"]');
  await expect(frame.getByRole('button', { name: 'Load image', exact: true })).toHaveCount(0);
  await expect(frame.getByRole('button', { name: 'Retry image', exact: true })).toHaveCount(3);
  const retry = frame.locator('[data-fairway-image="0"] button');
  await expect(retry).toHaveAttribute('title', 'Image server returned HTTP 404.');
  // Failed URLs do not automatically loop or retry when a different control is used.
  await page.getByRole('tab', { name: 'Text', exact: true }).click();
  await page.getByRole('tab', { name: 'HTML', exact: true }).click();
  await expect(frame.getByRole('button', { name: 'Retry image', exact: true })).toHaveCount(3);
  expect(JSON.parse((await page.locator('html').getAttribute('data-image-requests'))!)).toHaveLength(3);
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('test-mail-image-options', { detail: { delay: 0, fail: false } })));
  await retry.focus();
  await page.keyboard.press('Enter');
  await expect(frame.locator('img[data-fairway-loaded="0"]')).toHaveJSProperty('naturalWidth', 1);
  await expect(frame.getByRole('button', { name: 'Retry image', exact: true })).toHaveCount(2);
  expect(JSON.parse((await page.locator('html').getAttribute('data-image-requests'))!)).toHaveLength(4);
  expect(externalRequests.get(page)).toEqual([]);
});

test('late automatic downloads cannot overwrite another email or continue its discarded queue', async ({ page }) => {
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('test-mail-image-options', { detail: { delay: 400, fail: true } })));
  await page.getByRole('button', { name: 'View Mail', exact: true }).click();
  const emails = page.getByRole('list', { name: 'Emails', exact: true });
  await emails.getByRole('button', { name: /Boundary email/ }).click();
  await expect(page.locator('html')).toHaveAttribute('data-image-requests', JSON.stringify(['https://blocked.invalid/tracker']));
  await page.evaluate(() => window.dispatchEvent(new CustomEvent('test-mail-image-options', { detail: { delay: 0, fail: false } })));
  await emails.getByRole('button', { name: /Other email/ }).click();
  const frame = page.frameLocator('iframe[title="Email content"]');
  await expect(frame.locator('img[data-fairway-loaded]')).toHaveCount(3);
  await page.waitForTimeout(500);
  await expect(frame.getByRole('button', { name: 'Retry image', exact: true })).toHaveCount(0);
  await expect(frame.locator('img[data-fairway-loaded]')).toHaveCount(3);
  expect(JSON.parse((await page.locator('html').getAttribute('data-image-requests'))!)).toHaveLength(4);
  expect(externalRequests.get(page)).toEqual([]);
});
