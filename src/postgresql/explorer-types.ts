// Exact PostgreSQL data and structured error contracts; no bridge or persistence.
export type PgTarget = { kind: 'managed'; clusterId: string } | {
  kind: 'external'; host: string; port: number; user: string;
  /** Ephemeral native input only: never put this request in storage, history or logs. */
  password: string; transport: 'loopback' | 'verifyFull';
};
export type PgSession = { id: string; kind: 'managed' | 'external'; database: string; persistence: 'memoryOnly'; authentication: 'notChecked' };
export type PgPage = { limit?: number; offset?: number; sort?: { column: string; direction: 'asc' | 'desc' }[];
  filters?: { column: string; operator: 'eq' | 'ne' | 'lt' | 'lte' | 'gt' | 'gte' | 'contains' | 'isNull' | 'notNull'; value: string | null }[] };
export type PgOperation = { kind: 'databases' | 'schemas' }
  | { kind: 'relations'; schema: string }
  | { kind: 'columns' | 'indexes' | 'view'; schema: string; relation: string }
  | { kind: 'rows'; schema: string; relation: string; page?: PgPage }
  | { kind: 'query'; sql: string; parameters?: (string | null)[]; mode?: 'readOnly' | 'readWrite'; confirmed: true };
export type PgOutcome = 'notStarted' | 'notCommitted' | 'committed' | 'unknown';
export type PgColumn = { name: string; typeOid: number; typeName: string; typeModifier: number; tableOid: number; attributeNumber: number };
export type PgOutput = { columns: PgColumn[]; rows: (string | null)[][]; command: string; affectedRows: string | null;
  truncated: boolean; hasMore: boolean | null; warnings: string[]; outcome: PgOutcome };
export type PgError = { code: 'invalidRequest' | 'confirmationRequired' | 'unavailable' | 'connectionFailed' | 'cancelled' | 'timedOut' | 'queryFailed' | 'resultLimit' | 'busy' | 'staleSession';
  message: string; sqlState: string | null; position: number | null; outcome: PgOutcome };
export function isPgError(value: unknown): value is PgError {
  if (!value || typeof value !== 'object') return false;
  const e = value as Record<string, unknown>;
  return typeof e.message === 'string' && typeof e.code === 'string'
    && ['invalidRequest', 'confirmationRequired', 'unavailable', 'connectionFailed', 'cancelled', 'timedOut', 'queryFailed', 'resultLimit', 'busy', 'staleSession'].includes(e.code)
    && ['notStarted', 'notCommitted', 'committed', 'unknown'].includes(String(e.outcome))
    && (e.sqlState === null || typeof e.sqlState === 'string') && (e.position === null || typeof e.position === 'number');
}
