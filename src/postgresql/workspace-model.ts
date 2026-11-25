// UI lifecycle only: SQL enforcement, credentials and server ownership stay native.
import { isPgError, type PgError, type PgOperation, type PgOutput, type PgPage, type PgSession, type PgTarget } from './explorer-types.ts';
import { isPostgresError, type ManagedPostgresSnapshot } from './types.ts';
export type ConnectRequest = { confirmed: true; database: string; target: PgTarget };
export type Source = { kind: 'managed'; clusterId: string } | { kind: 'external'; host: string; port: number; user: string };
export type Connection = { session: PgSession; source: Source; access: 'notChecked' | 'authenticated' | 'failed' };
export type Pane = 'rows' | 'columns' | 'indexes' | 'view' | 'query';
export type ServiceAction = 'provision' | 'initialize' | 'start' | 'stop' | 'restart';
export type Problem = { text: string; code?: string; sqlState?: string | null; position?: number | null; outcome?: string };
export type Port = {
  snapshot(): Promise<ManagedPostgresSnapshot>; health(): Promise<ManagedPostgresSnapshot>;
  service(action: ServiceAction, port?: number): Promise<ManagedPostgresSnapshot>;
  open(request: ConnectRequest): Promise<PgSession>; close(sessionId: string): Promise<void>;
  execute(request: { jobId: string; sessionId: string; operation: PgOperation }): Promise<PgOutput>; cancel(jobId: string): Promise<void>;
};
export type State = {
  managed: ManagedPostgresSnapshot | null; serviceError: Problem | null;
  connections: Connection[]; activeId: string; databases: string[]; schemas: string[];
  schema: string; relations: { name: string; kind: string }[]; relation: string; pane: Pane;
  data: PgOutput | null; query: PgOutput | null; error: Problem | null; notice: string | null;
  pending: { label: string; cancelling: boolean; canCancel: boolean; query: boolean } | null;
};
export function problem(error: unknown): Problem {
  if (isPgError(error)) return { text: error.message, code: error.code, sqlState: error.sqlState, position: error.position, outcome: error.outcome };
  if (isPostgresError(error)) return { text: error.message, code: error.code };
  // Unknown exceptions may contain a serialized request. Never stringify them.
  return { text: 'The PostgreSQL request could not complete. Check the connection and retry.' };
}
export function catalogValues(output: PgOutput, name: string): string[] {
  const index = output.columns.findIndex(column => column.name === name);
  return index < 0 ? [] : output.rows.flatMap(row => row[index] === null || row[index] === undefined ? [] : [row[index]!]);
}
export function jobId() { return crypto.randomUUID().replaceAll('-', ''); }
const cancelled = (): PgError => ({ code: 'cancelled', message: 'Operation cancelled. No further requests were started.', sqlState: null, position: null, outcome: 'notStarted' });
type Task = { live: boolean; cancelled: boolean; job: string | null; session: string; write: boolean; canCancel: boolean };
export class PostgresWorkspace {
  private state: State = { managed: null, serviceError: null, connections: [], activeId: '', databases: [], schemas: [], schema: '', relations: [], relation: '', pane: 'rows', data: null, query: null, error: null, notice: null, pending: null };
  private listeners = new Set<() => void>();
  private task: Task | null = null;
  private disposed = false;
  private api: Port;
  constructor(api: Port) { this.api = api; }
  snapshot = () => this.state;
  subscribe = (fn: () => void) => { this.listeners.add(fn); return () => { this.listeners.delete(fn); }; };
  private patch(value: Partial<State>) { if (this.disposed) return; this.state = { ...this.state, ...value }; this.listeners.forEach(fn => fn()); }
  private clear(): Partial<State> { return { databases: [], schemas: [], schema: '', relations: [], relation: '', pane: 'rows', data: null, query: null, error: null }; }
  private access(id: string, access: Connection['access']) { this.patch({ connections: this.state.connections.map(c => c.session.id === id ? { ...c, access } : c) }); }
  private async run(label: string, action: (task: Task) => Promise<void>, write = false, clearIssue = true, canCancel = false): Promise<boolean> {
    if (this.task || this.disposed) return false;
    const task: Task = { live: true, cancelled: false, job: null, session: this.state.activeId, write, canCancel };
    this.task = task; this.patch({ pending: { label, cancelling: false, canCancel, query: false }, ...(clearIssue ? { error: null, notice: null } : {}) });
    try { await action(task); return task.live && !this.disposed; }
    catch (error) { if (task.live) this.patch({ error: problem(error) }); return false; }
    finally { if (this.task === task) { this.task = null; this.patch({ pending: null }); } }
  }
  private check(task: Task) { if (!task.live || task.cancelled || this.disposed) throw cancelled(); }
  private async request(task: Task, operation: PgOperation, keepOutcome = false): Promise<PgOutput> {
    this.check(task); const id = jobId(); task.job = id; task.canCancel = true;
    if (this.state.pending) this.patch({ pending: { ...this.state.pending, canCancel: true, query: true } });
    try {
      const result = await this.api.execute({ jobId: id, sessionId: task.session, operation });
      if (!task.live || this.disposed) throw cancelled();
      if (!keepOutcome) this.check(task);
      this.access(task.session, 'authenticated');
      return result;
    } catch (error) {
      if (task.live && isPgError(error) && ['connectionFailed', 'unavailable', 'staleSession'].includes(error.code)) this.access(task.session, 'failed');
      throw error;
    } finally { if (task.job === id) task.job = null; if (task.live && this.state.pending) this.patch({ pending: { ...this.state.pending, query: false } }); }
  }
  cancel() {
    const task = this.task; if (!task || !task.canCancel) return;
    task.cancelled = true; this.patch({ pending: { label: 'Cancelling PostgreSQL operation…', cancelling: true, canCancel: true, query: !!task.job } });
    if (task.job) void this.api.cancel(task.job).catch(() => {
      if (task.live) this.patch({ error: { text: 'Cancellation could not be delivered. The operation may still finish; wait for its outcome.', outcome: task.write ? 'unknown' : undefined } });
    });
  }
  private catalogNotice(output: PgOutput) { if (output.truncated) this.patch({ notice: 'Catalog limit reached. This list is incomplete; narrow the context or use SQL.' }); }
  private async load(task: Task) {
    const schemas = await this.request(task, { kind: 'schemas' }); this.patch({ schemas: catalogValues(schemas, 'name') }); this.catalogNotice(schemas);
    const databases = await this.request(task, { kind: 'databases' }); this.patch({ databases: catalogValues(databases, 'name') }); this.catalogNotice(databases);
  }
  connect(request: ConnectRequest) {
    // Copy only nonsecret metadata into state. The password is never retained here.
    const source: Source = request.target.kind === 'managed' ? { kind: 'managed', clusterId: request.target.clusterId } : { kind: 'external', host: request.target.host, port: request.target.port, user: request.target.user };
    return this.run('Connecting to PostgreSQL…', async task => {
      let session: PgSession;
      try { session = await this.api.open(request); }
      finally { if (request.target.kind === 'external') request.target.password = ''; }
      if (this.disposed || !task.live || task.cancelled) { await this.api.close(session.id); throw cancelled(); }
      task.session = session.id;
      this.patch({ ...this.clear(), activeId: session.id, connections: [...this.state.connections, { session, source, access: 'notChecked' }] });
      await this.load(task);
    }, false, true, true);
  }
  select(id: string) {
    if (!this.state.connections.some(c => c.session.id === id)) return Promise.resolve(false);
    return this.run('Loading PostgreSQL connection…', async task => { task.session = id; this.patch({ ...this.clear(), activeId: id }); await this.load(task); });
  }
  refresh() { return this.select(this.state.activeId); }
  async disconnect(id: string) {
    const task = this.task;
    const uncertain = task?.session === id && task.write;
    if (task?.session === id) { task.live = false; task.cancelled = true; if (task.job) void this.api.cancel(task.job).catch(() => {}); }
    if (id === this.state.activeId) this.patch({ ...this.clear(), notice: uncertain ? 'Disconnected during modifying SQL. Its outcome is unknown; verify the database before rerunning.' : null });
    try {
      await this.api.close(id);
      this.patch({ connections: this.state.connections.filter(c => c.session.id !== id), ...(id === this.state.activeId ? { activeId: '' } : {}) });
    } catch { this.patch({ error: { text: 'Could not close the native session. Retry Disconnect to release its session-only credential.', outcome: uncertain ? 'unknown' : undefined } }); }
  }
  schema(name: string) {
    if (!this.state.schemas.includes(name)) return Promise.resolve(false);
    return this.run('Loading tables and views…', async task => {
      this.patch({ schema: name, relation: '', relations: [], data: null, pane: 'rows' });
      const result = await this.request(task, { kind: 'relations', schema: name });
      const names = catalogValues(result, 'name'), kinds = catalogValues(result, 'kind');
      this.patch({ relations: names.map((name, index) => ({ name, kind: kinds[index] ?? '' })) }); this.catalogNotice(result);
    });
  }
  relation(name: string) {
    if (!this.state.relations.some(r => r.name === name)) return Promise.resolve(false);
    return this.run('Loading PostgreSQL rows…', async task => {
      this.patch({ relation: name, pane: 'rows', data: null });
      this.patch({ data: await this.request(task, { kind: 'rows', schema: this.state.schema, relation: name }) });
    });
  }
  pane(pane: Pane, page?: PgPage) {
    if (this.task) return Promise.resolve(false);
    if (pane === 'query') { this.patch({ pane, error: null }); return Promise.resolve(true); }
    if (!this.state.relation) { this.patch({ pane, data: null }); return Promise.resolve(true); }
    return this.run(`Loading PostgreSQL ${pane === 'view' ? 'view definition' : pane}…`, async task => {
      const { schema, relation } = this.state; this.patch({ pane, data: null });
      const operation: PgOperation = pane === 'rows' ? { kind: 'rows', schema, relation, page } : { kind: pane, schema, relation };
      this.patch({ data: await this.request(task, operation) });
    });
  }
  sql(sql: string, write: boolean) {
    return this.run(write ? 'Running authorized modifying SQL…' : 'Running read-only SQL…', async task => {
      this.patch({ pane: 'query', query: null });
      // A successful reply after Cancel can still prove a commit. Never relabel it.
      const result = await this.request(task, { kind: 'query', sql, mode: write ? 'readWrite' : 'readOnly', confirmed: true }, true);
      this.patch({ query: result, ...(task.cancelled ? { notice: 'Cancellation raced completion. The returned outcome is shown below; do not assume rollback.' } : {}) });
    }, write);
  }
  status(health = false) {
    return this.run(health ? 'Checking managed PostgreSQL access…' : 'Refreshing managed PostgreSQL status…', async () => {
      try { const managed = await (health ? this.api.health() : this.api.snapshot()); this.patch({ managed, serviceError: null }); }
      catch (error) { this.patch({ managed: null, serviceError: problem(error) }); }
    }, false, false);
  }
  service(action: ServiceAction, port?: number) {
    return this.run('Updating managed PostgreSQL…', async () => {
      try { this.patch({ managed: await this.api.service(action, port), serviceError: null, notice: action === 'provision' ? 'Runtime integrity verified. No cluster was initialized or started.' : 'Managed service action completed. Use Check access to verify authentication.' }); }
      catch (error) { this.patch({ serviceError: problem(error) }); }
    }, false, false);
  }
  dispose() {
    if (this.disposed) return; this.cancel(); this.disposed = true;
    if (this.task) this.task.live = false;
    this.state.connections.forEach(c => { void this.api.close(c.session.id).catch(() => {}); });
    this.listeners.clear();
  }
}
