import assert from 'node:assert/strict';
import test from 'node:test';
import { readFileSync } from 'node:fs';
import { isPostgresError } from '../src/postgresql/types.ts';
test('PostgreSQL discovery errors remain typed without accepting unrelated failures', () => {
  for (const code of ['busy', 'confirmationRequired', 'staleCandidate', 'unavailable']) {
    assert.ok(isPostgresError(JSON.parse(JSON.stringify({ code, message: 'Refresh discovery.' }))));
  }
  for (const value of [null, {}, new Error('error'), { code: 'saveFailed', message: 'error' }, { code: 'busy', message: 1 }]) assert.equal(isPostgresError(value), false);
});
test('Bridge keeps discovery separate from explicitly managed-only lifecycle', () => {
  const api = readFileSync(new URL('../src/postgresql/api.ts', import.meta.url), 'utf8');
  const commands = [...api.matchAll(/invoke\('([^']+)'/g)].map(match => match[1]);
  assert.deepEqual(commands, ['postgres_runtime_discover', 'postgres_runtime_inspect', ...['provision', 'initialize', 'start', 'stop', 'restart', 'snapshot', 'health', 'logs'].map(action => `postgres_managed_${action}`)]);
  const native = readFileSync(new URL('../src-tauri/src/desktop/postgresql_commands.rs', import.meta.url), 'utf8');
  for (const command of commands.slice(0, 2)) assert.ok(native.includes(`fn ${command}(`));
  const managed = readFileSync(new URL('../src-tauri/src/desktop/postgresql_managed_commands.rs', import.meta.url), 'utf8');
  for (const command of commands.slice(2)) assert.ok(managed.includes(command));
  assert.ok(managed.includes('super::begin_operation'));
  assert.equal(commands.some(command => /external|query|sql|explor/.test(command)), false);
  const rust = readFileSync(new URL('../src-tauri/src/postgresql.rs', import.meta.url), 'utf8');
  assert.ok(rust.includes('deny_unknown_fields'));
  assert.ok(rust.includes('pub confirmed: bool'));
});
test('Explorer bridge stays independent from managed lifecycle and exposes no persistence or backup commands', () => {
  const api = readFileSync(new URL('../src/postgresql/explorer.ts', import.meta.url), 'utf8');
  const commands = [...api.matchAll(/invoke\('([^']+)'/g)].map(match => match[1]);
  assert.deepEqual(commands, ['postgres_connection_open', 'postgres_connection_close', 'postgres_explorer_execute', 'postgres_explorer_cancel']);
  const native = readFileSync(new URL('../src-tauri/src/desktop/postgresql_explorer_commands.rs', import.meta.url), 'utf8');
  for (const command of commands) assert.ok(native.includes(`fn ${command}(`));
  assert.ok(native.includes('super::begin_operation'));
  assert.equal(/localStorage|sessionStorage|console\./.test(api), false);
});
