// No automatic invocation/polling or persistence. PostgreSQL values remain exact text/NULL.
import { invoke } from '../bridge';
import type { PgTarget, PgSession, PgOperation, PgOutput } from './explorer-types';
export function openPostgresConnection(request: { confirmed: true; database: string; target: PgTarget }): Promise<PgSession> {
  return invoke('postgres_connection_open', { request });
}
export function closePostgresConnection(sessionId: string): Promise<void> {
  return invoke('postgres_connection_close', { request: { sessionId } });
}
export function executePostgres(request: { jobId: string; sessionId: string; operation: PgOperation }): Promise<PgOutput> {
  return invoke('postgres_explorer_execute', { request });
}
export function cancelPostgres(jobId: string): Promise<void> {
  return invoke('postgres_explorer_cancel', { request: { jobId } });
}
