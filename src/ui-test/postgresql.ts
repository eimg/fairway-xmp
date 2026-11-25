// In-memory UI fixtures only. No credentials, SQL text, files or server processes retained.
import type { PgColumn, PgError, PgOperation, PgOutput, PgSession } from '../postgresql/explorer-types';
import type { ManagedPostgresSnapshot } from '../postgresql/types';
export const options: { fail?: string; delay?: string; delayMs?: number; empty?: boolean; openDelay?: number; cancelCommit?: boolean; ignoreCancel?: boolean; serviceFailure?: boolean } = {};
export function configure(value: typeof options) { Object.keys(options).forEach(key => delete options[key as keyof typeof options]); Object.assign(options, value); }
let counter = 0;
const sessions = new Map<string, PgSession>();
const jobs = new Map<string, { cancelled: boolean }>();
const early = new Set<string>();
const calls: { command: string; kind?: string; mode?: string; sessionId?: string }[] = [];
let managed: ManagedPostgresSnapshot = { supported: true, runtimeVersion: '18.6', clusterId: '1'.repeat(32), port: 15432, running: true, responsiveness: 'notChecked', authentication: 'notChecked', message: null };
export function inspect() { return { sessions: [...sessions.values()], calls: [...calls] }; }
const wait = (ms: number) => new Promise(resolve => setTimeout(resolve, ms));
function error(code: PgError['code'], message: string, outcome: PgError['outcome'] = 'notStarted'): PgError { return { code, message, sqlState: null, position: null, outcome }; }
function col(name: string, typeOid = 25, typeName = 'text'): PgColumn { return { name, typeOid, typeName, typeModifier: -1, tableOid: 0, attributeNumber: 0 }; }
function result(columns: PgColumn[], rows: PgOutput['rows'], extra: Partial<PgOutput> = {}): PgOutput { return { columns, rows, command: 'SELECT', affectedRows: String(rows.length), truncated: false, hasMore: null, warnings: [], outcome: 'notCommitted', ...extra }; }
export async function request(command: string, args: Record<string, unknown>): Promise<unknown> {
  const r = args.request as { jobId: string; sessionId: string; operation: PgOperation; confirmed: boolean; database: string; target: { kind: 'managed' | 'external' }; port: number } | undefined;
  calls.push({ command, kind: r?.operation?.kind, mode: r?.operation?.kind === 'query' ? r.operation.mode : undefined, sessionId: r?.sessionId });
  if (command === 'postgres_managed_snapshot') return structuredClone(managed);
  if (command === 'postgres_managed_health') { managed = { ...managed, responsiveness: 'accepting', authentication: options.serviceFailure ? 'rejected' : 'authenticated' }; return structuredClone(managed); }
  if (command.startsWith('postgres_managed_')) {
    if (options.serviceFailure) throw { code: 'unavailable', message: 'Secure Keychain credential unavailable; initialization did not continue.' };
    if (!r?.confirmed) throw error('confirmationRequired', 'Explicit authorization required');
    if (command.endsWith('initialize')) managed = { ...managed, clusterId: '1'.repeat(32), port: r.port, running: false };
    if (command.endsWith('start') || command.endsWith('restart')) managed = { ...managed, running: true, authentication: 'notChecked', responsiveness: 'notChecked' };
    if (command.endsWith('stop')) managed = { ...managed, running: false, authentication: 'notChecked', responsiveness: 'notChecked' };
    return structuredClone(managed);
  }
  if (command === 'postgres_connection_open') {
    if (options.openDelay) await wait(options.openDelay);
    if (!r?.confirmed) throw error('confirmationRequired', 'Authorize this connection');
    const session: PgSession = { id: (++counter).toString(16).padStart(32, '0'), kind: r.target.kind, database: r.database, persistence: 'memoryOnly', authentication: 'notChecked' };
    sessions.set(session.id, session); return session;
  }
  if (command === 'postgres_connection_close') { sessions.delete(r!.sessionId); return; }
  if (command === 'postgres_explorer_cancel') { const job = jobs.get(r!.jobId); if (job) job.cancelled = true; else early.add(r!.jobId); return; }
  if (command !== 'postgres_explorer_execute') throw error('unavailable', 'Unknown simulated command');
  if (!sessions.has(r!.sessionId)) throw error('staleSession', 'Connection closed');
  const op = r!.operation;
  const job = { cancelled: early.delete(r!.jobId) }; jobs.set(r!.jobId, job);
  try {
    if (options.delay === op.kind) await wait(options.delayMs ?? 1400);
    if (job.cancelled && !options.ignoreCancel) {
      if (op.kind === 'query' && op.mode === 'readWrite' && options.cancelCommit) return result([], [], { command: 'UPDATE 1', affectedRows: '1', outcome: 'committed' });
      throw error('cancelled', 'The PostgreSQL job was cancelled.', 'notCommitted');
    }
    if (options.fail === op.kind || options.fail === 'connection') throw error(options.fail === 'connection' ? 'connectionFailed' : options.fail === 'query' ? 'timedOut' : 'queryFailed', options.fail === 'query' ? 'Statement timeout reached.' : 'Fixture connection or catalog unavailable.', op.kind === 'query' && op.mode === 'readWrite' ? 'unknown' : 'notStarted');
    if (op.kind === 'schemas') return result([col('oid'), col('name')], options.empty ? [] : [['10', 'public'], ['11', 'tenant_日本']]);
    if (op.kind === 'databases') return result([col('oid'), col('name')], [['1', 'postgres'], ['2', 'practice 日本']]);
    if (op.kind === 'relations') return result([col('oid'), col('name'), col('kind')], options.empty ? [] : [['20', 'students_日本', 'r'], ['21', 'student_view', 'v'], ['22', 'empty_table', 'r']]);
    if (op.kind === 'columns') return result([col('name'), col('type'), col('not_null')], [['id', 'bigint', 't'], ['document', 'jsonb', 'f'], ['token', 'uuid', 'f'], ['tags', 'text[]', 'f'], ['stamp', 'timestamp with time zone', 'f']]);
    if (op.kind === 'indexes') return result([col('name'), col('primary'), col('unique'), col('definition')], [['students_pkey', 't', 't', 'CREATE UNIQUE INDEX students_pkey ON public.students_日本 (id)']]);
    if (op.kind === 'view') return result([col('name'), col('kind'), col('definition')], [['student_view', 'v', 'SELECT id, document FROM public.students_日本;']]);
    if (op.kind === 'query') {
      if (!op.confirmed) throw error('confirmationRequired', 'SQL execution requires authorization');
      if (op.mode === 'readWrite') return result([], [], { command: 'UPDATE 1', affectedRows: '1', outcome: 'committed' });
      return result([col('value'), col('value'), col('nothing')], [['9007199254740993', '', null]], { truncated: true, warnings: ['Result limit reached; statement cancelled and connection closed'] });
    }
    if (op.kind !== 'rows') throw error('invalidRequest', 'Unknown operation');
    const offset = op.page?.offset ?? 0;
    const filtered = !!op.page?.filters?.length;
    return result([col('id', 20, 'int8'), col('document', 3802, 'jsonb'), col('token', 2950, 'uuid'), col('tags', 1009, 'text[]'), col('stamp', 1184, 'timestamptz'), col('note')], options.empty || op.relation === 'empty_table' ? [] : [[filtered ? '7' : offset ? '51' : op.page?.sort?.[0]?.direction === 'desc' ? '100' : '9007199254740993', '{"big":9007199254740993,"html":"<script>bad</script>"}', '550e8400-e29b-41d4-a716-446655440000', '{"a,b",日本,NULL}', '2026-10-30 05:30:00+00', null]], { hasMore: !offset && !filtered && op.relation !== 'empty_table', warnings: ['Offset pages can change under concurrent writes'] });
  } finally { jobs.delete(r!.jobId); }
}
