import type { Page, TestInfo } from '@playwright/test';
import { spawn } from 'node:child_process';
import { createInterface } from 'node:readline';
import { join, resolve } from 'node:path';
import { mkdtemp, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';

// Test-only stdio transport, with isolated home/data and free service ports.
export async function nativeBridge(page: Page, info: TestInfo) {
  const binary = resolve('src-tauri/target/debug/examples/terminal_bridge' + (process.platform === 'win32' ? '.exe' : ''));
  // macOS Unix sockets have a 103-byte limit; Playwright's test-title paths exceed it.
  const parent = await mkdtemp(join(process.platform === 'darwin' ? '/tmp' : tmpdir(), 'fw-ui-'));
  const root = join(parent, 'isolated');
  const child = spawn(binary, [root, process.env.FAIRWAY_TEST_RUNTIME!], { stdio: ['pipe', 'pipe', 'pipe'] });
  let next = 0;
  let stderr = '';
  const timings: { command: string; operation?: string; elapsedMs: number; failed: boolean }[] = [];
  const pending = new Map<number, { resolve: (value: unknown) => void; reject: (reason: Error) => void; command: string; operation?: string; started: number }>();
  const lines = createInterface({ input: child.stdout });
  child.stderr.on('data', chunk => { stderr += chunk; });
  lines.on('line', line => {
    const response = JSON.parse(line);
    const item = pending.get(response.requestId); pending.delete(response.requestId);
    if (item && !item.command.startsWith('terminal_read')) timings.push({ command: item.command, operation: item.operation, elapsedMs: Date.now() - item.started, failed: typeof response.error === 'string' });
    if (typeof response.error === 'string') item?.reject(new Error(response.error)); else item?.resolve(response.value);
  });
  child.on('exit', code => {
    pending.forEach(item => item.reject(new Error(`Native bridge exited ${code}: ${stderr}`))); pending.clear();
  });
  await page.exposeFunction('nativeTerminalInvoke', (command: string, args: Record<string, unknown> = {}) => new Promise((resolve, reject) => {
    const requestId = ++next;
    const operation = (args.request as { op?: string } | undefined)?.op;
    pending.set(requestId, { resolve, reject, command, operation, started: Date.now() });
    child.stdin.write(JSON.stringify({ requestId, command, args }) + '\n');
  }));
  await page.addInitScript(() => {
    const bridge = window as unknown as { nativeTerminalInvoke: (command: string, args?: Record<string, unknown>) => Promise<unknown>; __TAURI_INTERNALS__: unknown };
    bridge.__TAURI_INTERNALS__ = { invoke: bridge.nativeTerminalInvoke, transformCallback: () => 1, unregisterCallback: () => {} };
  });
  return {
    errors: () => stderr,
    close: async () => {
      const exited = child.exitCode !== null ? Promise.resolve() : new Promise<void>(resolve => child.once('exit', () => resolve()));
      child.stdin.end();
      let timer: ReturnType<typeof setTimeout> | undefined;
      await Promise.race([exited, new Promise<void>(resolve => { timer = setTimeout(() => { child.kill(); resolve(); }, 15000); })]);
      clearTimeout(timer);
      lines.close();
      await info.attach('native-request-timings', { body: JSON.stringify(timings, null, 2), contentType: 'application/json' });
      for (const name of ['mysql.log', 'mysql-process.log', 'php.log']) {
        const log = await readFile(join(root, 'data 日本/logs', name)).catch(() => null);
        if (log) await info.attach(`native-${name}`, { body: log, contentType: 'text/plain' });
      }
      await rm(parent, { recursive: true, force: true });
    },
  };
}
