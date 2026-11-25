import assert from 'node:assert/strict';
import test from 'node:test';
import { PostgresWorkspace, jobId, problem, type Port } from '../src/postgresql/workspace-model.ts';
import type { PgOperation, PgOutput, PgSession, PgError } from '../src/postgresql/explorer-types.ts';
const output = (names: string[], rows: (string | null)[][] = []): PgOutput => ({ columns: names.map(name => ({ name, typeOid: 25, typeName: 'text', typeModifier: -1, tableOid: 0, attributeNumber: 0 })), rows, command: 'SELECT', affectedRows: null, truncated: false, hasMore: false, warnings: [], outcome: 'notCommitted' });
function deferred<T>() { let resolve!: (value: T) => void; let reject!: (value: unknown) => void; const promise = new Promise<T>((yes, no) => { resolve = yes; reject = no; }); return { promise, resolve, reject }; }
function fixture() {
  let count = 0;
  const closed: string[] = [], cancelled: string[] = [], operations: PgOperation[] = [];
  const port: Port = {
    open: async request => ({ id: (++count).toString(16).padStart(32, '0'), database: request.database, kind: request.target.kind, persistence: 'memoryOnly', authentication: 'notChecked' }),
    close: async id => { closed.push(id); }, cancel: async id => { cancelled.push(id); },
    execute: async request => { operations.push(request.operation); return request.operation.kind === 'schemas' ? output(['name'], [['public']]) : request.operation.kind === 'databases' ? output(['name'], [['postgres']]) : output(['id'], [['exact']]); },
    snapshot: async () => ({ supported: true, runtimeVersion: '18.6', clusterId: 'a'.repeat(32), port: 15432, running: true, responsiveness: 'notChecked', authentication: 'notChecked', message: null }),
    health: async () => { throw new Error('not used'); }, service: async () => { throw new Error('not used'); },
  };
  const model = new PostgresWorkspace(port);
  const connect = () => model.connect({ confirmed: true, database: 'postgres', target: { kind: 'managed', clusterId: 'a'.repeat(32) } });
  return { model, port, closed, cancelled, operations, connect };
}
test('PostgreSQL job identities are native 32-character hex and diagnostics never stringify unknown requests', () => {
  assert.match(jobId(), /^[0-9a-f]{32}$/);
  assert.equal(problem({ password: 'never-print-this', toString: () => { throw new Error('leak'); } }).text.includes('never-print-this'), false);
});
test('descriptors do not establish authentication; passwords stay out of controller state and are cleared after native open', async () => {
  const f = fixture(); const gate = deferred<PgOutput>(); f.port.execute = () => gate.promise;
  const request = { confirmed: true as const, database: 'postgres', target: { kind: 'external' as const, host: '127.0.0.1', port: 5432, user: 'fixture', password: 'private-marker', transport: 'loopback' as const } };
  const task = f.model.connect(request); await Promise.resolve(); await Promise.resolve();
  assert.equal(f.model.snapshot().connections[0].access, 'notChecked');
  assert.equal(JSON.stringify(f.model.snapshot()).includes('private-marker'), false);
  assert.equal(request.target.password, ''); gate.resolve(output(['name'], [])); await task;
  assert.equal(f.model.snapshot().connections[0].access, 'authenticated'); f.model.dispose();
});
test('dispose closes a descriptor arriving after unmount and never starts catalog jobs', async () => {
  const f = fixture(); const gate = deferred<PgSession>(); f.port.open = () => gate.promise;
  const task = f.connect(); f.model.dispose(); gate.resolve({ id: 'f'.repeat(32), kind: 'managed', database: 'postgres', persistence: 'memoryOnly', authentication: 'notChecked' }); await task;
  assert.deepEqual(f.closed, ['f'.repeat(32)]); assert.equal(f.operations.length, 0);
});
test('disconnect invalidates late rows and cancellation owns only the current job', async () => {
  const f = fixture(); await f.connect(); const gate = deferred<PgOutput>(); f.port.execute = () => gate.promise;
  const task = f.model.sql('SELECT 1', false); await f.model.disconnect(f.model.snapshot().activeId);
  gate.resolve(output(['old'], [['stale']])); await task;
  assert.equal(f.model.snapshot().query, null); assert.equal(f.model.snapshot().activeId, ''); assert.equal(f.cancelled.length, 1); assert.equal(f.closed.length, 1); f.model.dispose();
});
test('Cancel racing a commit preserves committed outcome and emits no follow-up query', async () => {
  const f = fixture(); await f.connect(); const gate = deferred<PgOutput>(); f.port.execute = () => gate.promise;
  const task = f.model.sql('UPDATE fixture SET value=1', true); f.model.cancel();
  gate.resolve({ ...output([], []), command: 'UPDATE 1', outcome: 'committed', affectedRows: '1' }); await task;
  assert.equal(f.model.snapshot().query?.outcome, 'committed'); assert.match(f.model.snapshot().notice!, /raced completion/); assert.equal(f.cancelled.length, 1); f.model.dispose();
});
test('unknown commit failures survive status refresh; pending read cancellation blocks follow-up catalogs', async () => {
  const f = fixture(); await f.connect();
  const error: PgError = { code: 'connectionFailed', message: 'Commit response unavailable', outcome: 'unknown', sqlState: null, position: null };
  f.port.execute = async () => { throw error; }; await f.model.sql('UPDATE fixture SET value=1', true); await f.model.status();
  assert.equal(f.model.snapshot().error?.outcome, 'unknown');
  const gate = deferred<PgOutput>(); let calls = 0; f.port.execute = () => { calls++; return gate.promise; };
  const task = f.model.refresh(); f.model.cancel(); gate.resolve(output(['name'], [['public']])); await task;
  assert.equal(calls, 1); assert.equal(f.model.snapshot().error?.code, 'cancelled'); f.model.dispose();
});
