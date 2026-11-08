import { useEffect, useEffectEvent, useRef, useState } from 'react';
import { ArrowUpRight, ChevronLeft, ChevronRight, Code2, Database, Eye, FolderOpen, Link2, LoaderCircle, Pencil, Plus, RefreshCw, Trash2 } from 'lucide-react';
import { invoke, listen } from '../bridge';
import Dialog from './Dialog';
import ColumnFields from './ColumnFields';
import RowFields, { initialInsertValues } from './RowFields';
import RowViewer from './RowViewer';
import { largeField, type CellChunk, type ReadCell } from './row-values';
import TableWorkspace from './TableWorkspace';
import ActionMenu from './ActionMenu';
import HelpTip from '../HelpTip';
import Workspace, { MaximizeWorkspace, useMaximizedWorkspace } from '../Workspace';
import IndexFields, { indexableColumns, suggestedIndexName, type IndexDefinition } from './IndexFields';
import SchemaDetails from './SchemaDetails';
import ForeignKeyExamples from './ForeignKeyExamples';
import SqlBlock from './SqlBlock';
import { summarizeQueryResults } from './query-results';
import { editableColumn, fromColumn, newColumn, type Column, type Definition, type LargeFields, type RowRef, type Page, type QueryResult, type Row, type Structure, type Table, type TableIndex } from './types';
import './data.css';

type EditKind = 'import_sql' | 'create_database' | 'rename_database' | 'drop_database' | 'create_table' | 'rename_table' | 'drop_table' | 'add_column' | 'change_column' | 'drop_column' | 'create_index' | 'drop_index' | 'insert' | 'update' | 'delete';
type Edit = { kind: EditKind; path?: string; row?: Row; column?: Column; index?: TableIndex; rowRef?: RowRef; largeFields?: LargeFields };
const titles: Record<EditKind, string> = { import_sql: 'Import SQL', create_database: 'Create database', rename_database: 'Rename database', drop_database: 'Delete database', create_table: 'Create table', rename_table: 'Rename table', drop_table: 'Delete table', add_column: 'Add column', change_column: 'Edit column', drop_column: 'Delete column', create_index: 'Create index', drop_index: 'Remove index', insert: 'Insert row', update: 'Edit row', delete: 'Delete row' };
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);
const idColumn = (): Definition => ({ ...newColumn('id'), type: 'BIGINT', nullable: false, primary: true, autoIncrement: true });
const cell = (value: string | null) => value === null ? <span className="db-null">NULL</span> : value === '' ? <span className="db-null">empty</span> : value;
type Source = { driver: 'mysql' } | { driver: 'sqlite'; path: string | null };
const recentKey = 'fairway.sqlite.recent.v1';
const missingSqliteFile = 'This SQLite file is missing or is not a file.';
const fileName = (path: string) => path.split(/[/\\]/).pop() || path;
function recentFiles(): string[] {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(recentKey) ?? '[]');
    return Array.isArray(value) ? value.filter((path): path is string => typeof path === 'string' && path.length > 0).slice(0, 6) : [];
  } catch { return []; }
}

export default function DataBrowser({ ready, busy: appBusy, start, open }: { ready: boolean; busy: boolean; start: () => void; open: (target: string) => void }) {
  const maximized = useMaximizedWorkspace() === 'data';
  const [source, setSource] = useState<Source>({ driver: 'mysql' });
  const selectedSource = useRef(source);
  const [recent, setRecent] = useState(recentFiles);
  const sqlite = source.driver === 'sqlite';
  const sqlitePath = sqlite ? source.path : null;
  const available = !!sqlitePath || (!sqlite && ready);
  const [databases, setDatabases] = useState<string[]>([]);
  const [catalogLoaded, setCatalogLoaded] = useState(false);
  const [database, setDatabase] = useState('');
  const [tables, setTables] = useState<Table[]>([]);
  const [table, setTable] = useState('');
  const [page, setPage] = useState<Page | null>(null);
  const [structure, setStructure] = useState<Structure | null>(null);
  const [tableError, setTableError] = useState<string | null>(null);
  const [pane, setPane] = useState<'rows' | 'structure' | 'query'>('rows');
  const [pending, setPending] = useState<string | null>(null);
  const [importPhase, setImportPhase] = useState<'working' | 'backup' | 'import' | 'canceling' | 'refreshing' | null>(null);
  const [notice, setNotice] = useState<{ error: boolean; text: string } | null>(null);
  const [edit, setEdit] = useState<Edit | null>(null);
  const [foreignKeyExamples, setForeignKeyExamples] = useState<'create' | 'structure' | null>(null);
  const [tableSql, setTableSql] = useState<{ table: string; sql: string } | null>(null);
  const [viewedRow, setViewedRow] = useState<{ row: Row; columns: Column[]; table: string; database: string; source: Source; position: number; rowRef?: RowRef; largeFields?: LargeFields } | null>(null);
  const [name, setName] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [definitions, setDefinitions] = useState<Definition[]>([]);
  const [fieldLoading, setFieldLoading] = useState(false);
  const [values, setValues] = useState<Row>({});
  const [indexDefinition, setIndexDefinition] = useState<IndexDefinition>({ name: '', unique: false, columns: [] });
  const [sort, setSort] = useState({ sort: '', direction: 'ASC' });
  const [sql, setSql] = useState('SELECT VERSION();');
  const [query, setQuery] = useState<QueryResult | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const editor = useRef<HTMLTextAreaElement>(null);
  const active = useRef(false);
  const activeTask = useRef<Promise<void> | null>(null);
  const job = useRef<string | null>(null);
  const mounted = useRef(false);
  const restoreRecent = useRef(true);
  const operationFocus = useRef<HTMLElement | null>(null);
  const rowsScroll = useRef<HTMLDivElement>(null);
  const mysqlSelection = useRef({ database: '', table: '' });
  const [rowsHeight, setRowsHeight] = useState<number>();
  const browsing = !!pending && /^(Loading|Refreshing|Sorting) /.test(pending);
  const [showPending, setShowPending] = useState(false);
  const blocked = (!!pending && !browsing) || appBusy;
  const writable = page?.kind === 'BASE TABLE';
  const canEdit = writable && !!page?.key.length;

  async function request<T>(body: Record<string, unknown>, targetSource = source, signal?: AbortSignal): Promise<T> {
    signal?.throwIfAborted();
    const id = crypto.randomUUID(); job.current = id;
    const cancel = () => { void invoke('cancel_data_browser', { id }).catch(() => {}); };
    signal?.addEventListener('abort', cancel, { once: true });
    try { return await invoke<T>('data_browser', { id, request: { ...body, ...targetSource } }); }
    finally { signal?.removeEventListener('abort', cancel); if (job.current === id) job.current = null; }
  }
  function cellReader(db: string, tbl: string, target: Source, rowRef?: RowRef): ReadCell | undefined {
    return rowRef ? (column, offset, signal) => request<CellChunk>({ op: 'cell', database: db, table: tbl, rowRef, column, offset }, target, signal) : undefined;
  }
  async function run(label: string, action: () => Promise<void>) {
    if (active.current) return;
    operationFocus.current = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    active.current = true; setPending(label); setNotice(null);
    const task = (async () => {
      try { await action(); }
      catch (error) { if (mounted.current) setNotice({ error: true, text: errorText(error) }); }
      finally { active.current = false; setPending(null); }
    })();
    activeTask.current = task;
    await task;
    if (activeTask.current === task) activeTask.current = null;
  }
  async function loadRows(db: string, selected: string, offset = 0, order = sort) {
    if (!selected) { setPage(null); return; }
    const result = await request<Page>({ op: 'rows', database: db, table: selected, offset, ...(order.sort ? order : {}) });
    setPage(result);
  }
  async function load(preferredDb = database || (!sqlite ? mysqlSelection.current.database : ''), preferredTable = table || (!sqlite ? mysqlSelection.current.table : ''), targetSource = source) {
    const list = await request<{ databases: string[] }>({ op: 'databases' }, targetSource);
    const db = list.databases.includes(preferredDb) ? preferredDb : (list.databases[0] ?? '');
    const result = db ? await request<{ tables: Table[] }>({ op: 'tables', database: db }, targetSource) : { tables: [] };
    const selected = result.tables.find(t => t.name === preferredTable)?.name ?? result.tables[0]?.name ?? '';
    let nextPage: Page | null = null;
    let nextStructure: Structure | null = null;
    let failure: string | null = null;
    try {
      if (selected) {
        nextPage = await request<Page>({ op: 'rows', database: db, table: selected, offset: 0 }, targetSource);
        nextStructure = await request<Structure>({ op: 'structure', database: db, table: selected }, targetSource);
      }
    } catch (error) {
      if (!mounted.current) throw error;
      nextPage = null; nextStructure = null;
      failure = errorText(error);
    }
    // A table failure must not hide the catalog or SQL repair tools. Publish
    // the new selection with empty data rather than pairing it with old rows.
    if (db !== database) setQuery(null);
    setDatabases(list.databases); setDatabase(db); setTables(result.tables);
    setTable(selected); setPage(nextPage); setStructure(nextStructure);
    setTableError(failure);
    setCatalogLoaded(true);
    setSort({ sort: '', direction: 'ASC' });
    if (targetSource.driver === 'mysql') mysqlSelection.current = { database: db, table: selected };
  }
  function remember(paths: string[]) {
    setRecent(paths);
    try { localStorage.setItem(recentKey, JSON.stringify(paths)); } catch { /* Browsing still works without local storage. */ }
  }
  async function switchSource(next: Source) {
    const previous = selectedSource.current;
    selectedSource.current = next;
    try {
      const opened = next.driver === 'sqlite' ? next.path : null;
      if ((next.driver === 'mysql' && !ready) || (next.driver === 'sqlite' && !opened)) {
        setDatabases([]); setDatabase(''); setTables([]); setTable(''); setPage(null); setStructure(null); setCatalogLoaded(false);
        setTableError(null);
      } else await load(next.driver === 'mysql' ? mysqlSelection.current.database : '', next.driver === 'mysql' ? mysqlSelection.current.table : '', next);
      setSource(next); setPane('rows'); setQuery(null); setHistory([]);
      setSql(next.driver === 'sqlite' ? 'SELECT sqlite_version();' : 'SELECT VERSION();');
      if (opened) {
        restoreRecent.current = true;
        remember([opened, ...recent.filter(path => path !== opened)].slice(0, 6));
      }
    } catch (error) {
      selectedSource.current = previous;
      throw error;
    }
  }
  function selectMysql() {
    if (!sqlite) return;
    void run('Loading database…', () => switchSource({ driver: 'mysql' }));
  }
  function selectSqlite() {
    if (sqlite) return;
    void run('Loading database…', () => openRecentSqlite(restoreRecent.current ? recent[0] ?? null : null));
  }
  async function openRecentSqlite(path: string | null) {
    try {
      await switchSource({ driver: 'sqlite', path });
    } catch (error) {
      if (path && errorText(error).startsWith(missingSqliteFile)) {
        restoreRecent.current = false;
        remember(recent.filter(saved => saved !== path));
        await switchSource({ driver: 'sqlite', path: null });
      }
      throw error;
    }
  }
  async function openSqlite() {
    await run('Opening SQLite file…', async () => {
      const path = await invoke<string | null>('pick_sqlite_file');
      if (path) await switchSource({ driver: 'sqlite', path });
    });
  }
  function reopenSqlite() {
    const path = recent[0];
    if (!path) return;
    void run('Opening SQLite file…', () => openRecentSqlite(path));
  }
  function backupSqlite() {
    void run('Backing up SQLite file…', async () => {
      const result = await request<{ message: string }>({ op: 'backup' });
      setNotice({ error: false, text: result.message });
    });
  }
  function closeSqlite() {
    restoreRecent.current = false;
    void switchSource({ driver: 'sqlite', path: null });
  }
  useEffect(() => {
    if (pending || !operationFocus.current) return;
    const trigger = operationFocus.current;
    operationFocus.current = null;
    // Disabling the control can blur it. Restore it without scrolling, and
    // leave focus alone when the user has moved on or a dialog is open.
    if (document.activeElement === document.body && !document.querySelector('[aria-modal="true"]') && trigger.isConnected && !trigger.matches(':disabled')) {
      trigger.focus({ preventScroll: true });
    }
  }, [pending]);
  useEffect(() => {
    setShowPending(false);
    if (!pending) return;
    if (!browsing) { setShowPending(true); return; }
    const timer = window.setTimeout(() => setShowPending(true), 200);
    return () => window.clearTimeout(timer);
  }, [pending, browsing]);
  const refreshCurrentSource = useEffectEvent(() => {
    // Read the current source after waiting for an active task; never publish
    // a MySQL catalog into a SQLite workspace. Keep refresh on sidebar return.
    // A switch can finish before React commits setSource. Its synchronous
    // selection also protects this gap between task completion and rendering.
    const target = selectedSource.current;
    if (target.driver === 'sqlite' ? target.path : ready) void run('Loading databases…', () => load(undefined, undefined, target));
    else { setPage(null); setStructure(null); }
  });
  useEffect(() => {
    let disposed = false;
    // A sidebar return may overlap cancellation of the previous view's request.
    // Wait for it before refreshing, so an initial load cannot be skipped forever.
    void (async () => {
      await activeTask.current;
      if (disposed) return;
      refreshCurrentSource();
    })();
    return () => { disposed = true; };
    // Refresh on visibility/service changes, not on each snapshot poll.
  }, [ready]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      // React's development effect replay is not an actual navigation away.
      queueMicrotask(() => { if (!mounted.current && job.current) void invoke('cancel_data_browser', { id: job.current }).catch(() => {}); });
    };
  }, []);
  useEffect(() => {
    const element = rowsScroll.current;
    if (!element || maximized) return;
    const measure = () => {
      const below = element.nextElementSibling instanceof HTMLElement ? element.nextElementSibling.offsetHeight : 0;
      const room = window.innerHeight - element.getBoundingClientRect().top - below - 20;
      setRowsHeight(Math.max(220, Math.floor(room)));
    };
    measure();
    const observer = new ResizeObserver(measure);
    if (element.parentElement) observer.observe(element.parentElement);
    window.addEventListener('resize', measure);
    return () => { observer.disconnect(); window.removeEventListener('resize', measure); };
  }, [pane, table, database, sqlite, page, catalogLoaded, maximized]);

  function begin(kind: EditKind, row?: Row, column?: Column, index?: TableIndex, rowRef?: RowRef, largeFields?: LargeFields) {
    setNotice(null); setName(kind === 'rename_database' ? database : kind === 'rename_table' ? table : '');
    setFieldLoading(false); setConfirmation(''); setValues(kind === 'insert' && page ? initialInsertValues(page.columns) : {});
    const fresh = (name = ''): Definition => ({ ...newColumn(name, sqlite), ...(sqlite ? { type: 'TEXT' } : {}) });
    setDefinitions(kind === 'create_table' ? [{ ...idColumn(), ...(sqlite ? { type: 'INTEGER' } : {}) }, fresh('name')] : column ? [fromColumn(column)] : [fresh()]);
    const first = indexableColumns(page?.columns ?? [], sqlite)[0]?.name;
    setIndexDefinition({ name: suggestedIndexName(table, first ? [first] : [], false), unique: false, columns: first ? [first] : [] });
    setEdit({ kind, row, column, index, rowRef, largeFields });
  }
  async function save() {
    if (!edit || fieldLoading) return;
    const current = edit;
    await run('Saving database changes…', async () => {
      const body: Record<string, unknown> = { op: current.kind, database, table, confirmed: true };
      if (current.kind === 'create_database') body.database = name;
      if (['rename_database', 'rename_table', 'drop_column'].includes(current.kind)) body.name = current.kind === 'drop_column' ? current.column?.name : name;
      if (current.kind === 'create_table') { body.table = name; body.columns = definitions; }
      if (current.kind === 'add_column' || current.kind === 'change_column') { body.column = definitions[0]; body.oldName = current.column?.name; }
      if (current.rowRef) body.rowRef = current.rowRef;
      else if (current.row) { body.original = current.row; body.binaryColumns = page?.columns.filter(c => c.binary).map(c => c.name); }
      if (current.kind === 'insert' || current.kind === 'update') body.values = values;
      if (current.kind === 'create_index') { body.name = indexDefinition.name; body.indexColumns = indexDefinition.columns; body.unique = indexDefinition.unique; }
      if (current.kind === 'drop_index') { body.name = current.index?.name; body.originalIndex = current.index; }
      const result = await request<{ message: string }>(body);
      setEdit(null);
      await load(current.kind === 'create_database' || current.kind === 'rename_database' ? name : database, current.kind === 'create_table' || current.kind === 'rename_table' ? name : table);
      setNotice({ error: false, text: result.message });
    });
  }
  function showPane(next: typeof pane) {
    setPane(next);
  }
  async function chooseImport() {
    await run('Choosing SQL file…', async () => {
      const path = await invoke<string | null>('pick_sql_file');
      if (path) { setConfirmation(''); setEdit({ kind: 'import_sql', path }); }
    });
  }
  async function importSql() {
    if (!edit?.path || confirmation !== database || sqlite) return;
    const path = edit.path;
    await run('Importing SQL…', async () => {
      setImportPhase('working');
      const unlisten = await listen('data-browser-progress', payload => {
        const progress = payload as { id?: string; phase?: string } | null;
        if (progress?.id === job.current && (progress?.phase === 'backup' || progress?.phase === 'import')) {
          const phase = progress.phase;
          setImportPhase(previous => previous === 'canceling' ? previous : phase);
        }
      }).catch(() => () => {});
      let result: { message: string };
      try {
        result = await request<{ message: string }>({ op: 'import', database, sqlFile: path, confirmed: true });
      } catch (error) {
        setImportPhase('refreshing');
        setConfirmation(''); setQuery(null);
        // A failed/canceled import may have committed earlier statements.
        try { if (mounted.current) await load(); } catch { /* Preserve the import error and backup path. */ }
        throw error;
      } finally { unlisten(); setImportPhase(null); }
      setEdit(null); setQuery(null);
      try { await load(); }
      catch (error) { setNotice({ error: true, text: `${result.message}\nImport completed, but the browser could not refresh: ${errorText(error)}` }); return; }
      setNotice({ error: false, text: result.message });
    });
  }
  function cancelImport() {
    const id = job.current;
    if (!id) return;
    const previousPhase = importPhase;
    setImportPhase('canceling');
    void invoke('cancel_data_browser', { id }).catch(error => {
      setNotice({ error: true, text: errorText(error) });
      if (job.current === id) setImportPhase(previousPhase);
    });
  }
  async function exportSql(selectedTable?: string) {
    await run(selectedTable ? 'Exporting table…' : 'Exporting database…', async () => {
      const result = await request<{ message: string }>({ op: 'export', database, ...(selectedTable ? { table: selectedTable } : {}) });
      setNotice({ error: false, text: result.message });
    });
  }
  async function runQuery() {
    const selection = editor.current;
    const text = selection && selection.selectionStart !== selection.selectionEnd ? sql.slice(selection.selectionStart, selection.selectionEnd) : sql;
    setQuery(null);
    await run('Running SQL…', async () => {
      const result = await request<QueryResult>({ op: 'query', database, sql: text });
      setQuery(summarizeQueryResults(text, result, sqlite)); setHistory(previous => [text, ...previous.filter(item => item !== text)].slice(0, 10));
      // SQL can change or remove the selected table. Refresh the catalog after execution.
      await load();
    });
  }
  const destructive = edit?.kind.startsWith('drop_') || edit?.kind === 'delete';
  const target = edit?.kind === 'drop_database' ? database : edit?.kind === 'drop_table' ? table : edit?.kind === 'drop_index' ? edit.index?.name ?? '' : edit?.column?.name ?? '';
  const needsName = edit && ['create_database', 'rename_database', 'create_table', 'rename_table'].includes(edit.kind);
  const columnForm = edit && ['add_column', 'change_column'].includes(edit.kind);
  const pageNotice = notice && !edit ? notice : null;

  const headingActions = <div className="db-actions" inert={browsing}><button className="secondary" disabled={blocked} onClick={() => void openSqlite()}><FolderOpen size={14} />Open SQLite file…</button>{!sqlite && <button className="secondary" disabled={blocked || !ready} onClick={() => open('phpmyadmin')}>phpMyAdmin<ArrowUpRight size={14} /></button>}</div>;
  return <Workspace id="data" title="Data browser" className="db-browser" busy={!!pending} actions={headingActions}>
    <div className="page-heading"><div><h1>Data browser</h1><p>Query and edit MySQL databases or SQLite files.</p></div><div className="workspace-heading-actions">{headingActions}<MaximizeWorkspace id="data" title="Data browser" /></div></div>
    <div className={`db-notice${pageNotice ? ` notice ${pageNotice.error ? 'error' : 'success'}` : ''}`}>
      <div className="db-notice-message">
        <div role={pageNotice && !pageNotice.error ? 'status' : undefined} aria-live="polite" aria-atomic="true">{pageNotice && !pageNotice.error ? pageNotice.text : null}</div>
        <div role={pageNotice?.error ? 'alert' : undefined} aria-live="assertive" aria-atomic="true">{pageNotice?.error ? pageNotice.text : null}</div>
      </div>
      {pageNotice && <button aria-label="Dismiss database message" onClick={() => setNotice(null)}>×</button>}
    </div>
    <section className="db-source" inert={browsing} aria-label="Data source controls">
      <div className="db-source-tabs" role="tablist" aria-label="Data source"><button role="tab" aria-selected={!sqlite} disabled={blocked} onClick={selectMysql}>MySQL</button><button role="tab" aria-selected={sqlite} disabled={blocked} onClick={selectSqlite}>SQLite</button></div>
      {sqlitePath && <section className="db-file-card" aria-label="SQLite file"><p className="db-file-path" title={sqlitePath}><strong>{fileName(sqlitePath)}</strong><span aria-label="SQLite file path">{sqlitePath}</span><small>Editing the original project file. Changes are saved immediately.</small></p><div className="db-file-actions"><button className="secondary" disabled={blocked} onClick={backupSqlite}>Back up SQLite file</button><button className="secondary" disabled={blocked} onClick={closeSqlite}>Close SQLite file</button></div></section>}
    </section>
    {sqlite && !sqlitePath ? <section className="settings-card db-empty"><FolderOpen size={28} /><h2>Open a SQLite file to browse your data</h2><p>Choose a project database. SQLite needs no running services.</p><div className="db-empty-actions"><button className="primary" disabled={blocked} onClick={() => void openSqlite()}>Open SQLite file</button>{recent[0] && <button className="secondary" disabled={blocked} title={recent[0]} onClick={() => void reopenSqlite()}>Reopen {fileName(recent[0])}</button>}</div></section> : !available ? <section className="settings-card db-empty"><Database size={28} /><h2>Start MySQL to browse your data</h2><p>Or choose <strong>Open SQLite file…</strong> above to browse a project database. SQLite needs no running services.</p><button className="primary" disabled={blocked} onClick={start}>Start MySQL</button></section> : <div className="db-content" inert={browsing}>
      {!sqlite && <>
      <section className="db-catalog" aria-label="Database controls">
        <label>Database<select aria-label="Database" value={database} disabled={blocked || !databases.length} onChange={e => void run('Loading database…', () => load(e.target.value, ''))}>{!databases.length && <option value="">No databases</option>}{databases.map(db => <option key={db}>{db}</option>)}</select></label>
        <div className="db-actions"><button className="secondary" disabled={blocked} onClick={() => begin('create_database')}><Plus size={14} />Database</button><button className="icon-button" aria-label="Refresh databases" disabled={blocked} onClick={() => void run('Refreshing databases…', () => load())}><RefreshCw size={15} /></button><ActionMenu label="Database actions" disabled={blocked || !database} actions={[
          { label: 'Export database SQL', onSelect: () => void exportSql() },
          { label: 'Import SQL file…', onSelect: () => void chooseImport() },
          { label: 'Rename database', onSelect: () => begin('rename_database') },
          { label: 'Delete database', danger: true, onSelect: () => begin('drop_database') },
        ]} /></div>
      </section>
      </>}
      {!catalogLoaded && !notice?.error && <div className="db-loading-placeholder" aria-label="Loading database workspace" />}
      {database && <TableWorkspace sidebar={
        <aside className="db-tables" aria-label="Tables"><div className="db-tables-heading"><h2>Tables <span>{tables.length}</span></h2><button className="icon-button" aria-label="Create table" title="Create table" disabled={blocked} onClick={() => begin('create_table')}><Plus size={16} /></button></div>
          {tables.length ? tables.map(t => <button key={t.name} title={t.name} className={`db-table-choice ${table === t.name ? 'selected' : ''}`} disabled={blocked} onClick={() => void run('Loading table…', () => load(database, t.name))}><span>{t.name}</span>{t.kind === 'VIEW' && <small>view</small>}</button>) : <div className="db-tables-empty"><p className="db-hint">No tables yet.</p><button className="secondary" disabled={blocked} onClick={() => begin('create_table')}>Create table</button></div>}
        </aside>
      }>{toggle =>
        <section className="db-detail" aria-label="Database workspace">
          <div className="db-detail-heading"><div className="db-detail-title">{toggle}<h2>{table || (sqlite ? fileName(database) : database)}</h2></div><div className="db-detail-controls">{table && <>{pane === 'rows' && <button className="secondary" disabled={blocked || !writable} onClick={() => begin('insert')}><Plus size={14} />Insert row</button>}<ActionMenu label="Table actions" disabled={blocked} actions={[
            ...(!sqlite ? [{ label: 'Export table SQL', onSelect: () => void exportSql(table) }] : []),
            { label: 'Rename table', disabled: !writable, onSelect: () => begin('rename_table') },
            { label: 'Delete table', disabled: !writable, danger: true, onSelect: () => begin('drop_table') },
          ]} /></>}</div></div>
          <div className="db-tabs" role="tablist" aria-label="Data views">{(['rows', 'structure', 'query'] as const).map(tab => <button key={tab} role="tab" aria-selected={pane === tab} disabled={blocked || (tab !== 'query' && !table)} onClick={() => showPane(tab)}>{tab === 'query' ? 'SQL query' : tab === 'rows' ? 'Rows' : 'Structure'}</button>)}</div>
          {tableError && <div className="notice error" role="alert"><span>Could not load {table}. {tableError} Choose another table or use SQL to repair it.</span><button className="secondary" disabled={blocked} onClick={() => void run('Refreshing table…', () => load())}>Retry table</button></div>}
          {!table && pane !== 'query' ? <div className="db-empty"><p>Create a table or open SQL query to begin.</p><div className="db-empty-actions"><button className="secondary" disabled={blocked} onClick={() => begin('create_table')}>Create table</button><button className="secondary" disabled={blocked} onClick={() => showPane('query')}>Open SQL query</button></div></div> : pane === 'rows' && page ? <>
            {!canEdit && <p className="db-hint">{writable ? 'No non-null primary or unique key: browse and insert are available. Use SQL for row updates and deletes.' : page.kind === 'VIEW' ? 'This view is read-only. Use SQL for advanced changes.' : 'This virtual table is read-only in forms. Use SQL for advanced changes.'}</p>}
            <div ref={rowsScroll} className="db-grid-scroll" tabIndex={0} role="region" aria-label="Table rows" style={!maximized && rowsHeight ? { maxHeight: rowsHeight } : undefined}><table className="db-grid"><thead><tr><th>Actions</th>{page.columns.map(c => <th key={c.name}><button disabled={blocked} onClick={() => { const next = { sort: c.name, direction: sort.sort === c.name && sort.direction === 'ASC' ? 'DESC' : 'ASC' }; setSort(next); void run('Sorting rows…', () => loadRows(database, table, 0, next)); }}>{c.name}{sort.sort === c.name ? sort.direction === 'ASC' ? ' ↑' : ' ↓' : ''}</button></th>)}</tr></thead><tbody>{page.rows.map((row, index) => <tr key={index}><td className="db-actions-cell"><div className="db-row-actions"><button className="icon-button" title={`View row ${index + 1}`} aria-label={`View row ${index + 1}`} disabled={blocked} onClick={() => { setNotice(null); setViewedRow({ row, columns: page.columns, table, database, source, position: page.offset + index + 1, rowRef: page.rowRefs?.[index], largeFields: page.truncated?.[index] }); }}><Eye size={14} /></button>{canEdit && <><button className="icon-button" title={`Edit row ${index + 1}`} aria-label={`Edit row ${index + 1}`} disabled={blocked} onClick={() => begin('update', row, undefined, undefined, page.rowRefs?.[index], page.truncated?.[index])}><Pencil size={14} /></button><button className="icon-button db-icon-danger" title={`Delete row ${index + 1}`} aria-label={`Delete row ${index + 1}`} disabled={blocked} onClick={() => begin('delete', row, undefined, undefined, page.rowRefs?.[index], page.truncated?.[index])}><Trash2 size={14} /></button></>}</div></td>{page.columns.map(c => <td key={c.name}><span className="db-cell" title={largeField(page.truncated?.[index], c.name) ? `Preview · ${largeField(page.truncated?.[index], c.name)!.length.toLocaleString()} characters. Open the row for the complete value.` : row[c.name] ?? 'SQL NULL'}>{cell(row[c.name])}{largeField(page.truncated?.[index], c.name) && '…'}</span></td>)}</tr>)}</tbody></table>{!page.rows.length && <p className="db-grid-empty">{writable ? 'No rows yet. Insert a row to get started.' : 'This view has no rows.'}</p>}</div>
            <div className="db-pagination"><span>{page.rows.length ? page.offset + 1 : 0}–{page.offset + page.rows.length} of {page.total}</span><div className="db-actions"><button className="icon-button" aria-label="Previous page" disabled={blocked || page.offset === 0} onClick={() => void run('Loading rows…', () => loadRows(database, table, Math.max(0, page.offset - page.limit)))}><ChevronLeft size={17} /></button><button className="icon-button" aria-label="Next page" disabled={blocked || BigInt(page.offset + page.rows.length) >= BigInt(page.total)} onClick={() => void run('Loading rows…', () => loadRows(database, table, page.offset + page.limit))}><ChevronRight size={17} /></button><button className="icon-button" aria-label="Refresh rows" disabled={blocked} onClick={() => void run('Refreshing rows…', () => loadRows(database, table, page.offset))}><RefreshCw size={14} /></button></div></div>
          </> : pane === 'structure' && page ? <>
            <div className="db-row-tools"><p className="db-hint">{page.key.length ? `Row identity: ${page.key.join(', ')}` : 'No safe row identity found.'}</p><div className="db-actions"><button type="button" className="secondary" disabled={blocked || !structure?.sql} onClick={() => { if (structure?.sql) setTableSql({ table, sql: structure.sql }); }}><Code2 size={14} />Table SQL</button><button className="secondary" disabled={blocked || !writable} onClick={() => begin('add_column')}><Plus size={14} />Column</button></div></div>
            <div className="db-grid-scroll" tabIndex={0} role="region" aria-label="Table structure"><table className="db-grid"><thead><tr><th>Column</th><th>Type</th><th>NULL</th><th>Default</th><th>Attributes</th><th>Actions</th></tr></thead><tbody>{page.columns.map(c => <tr key={c.name}><td>{c.name}</td><td>{c.type}</td><td>{c.nullable ? 'Yes' : 'No'}</td><td>{cell(c.defaultValue)}</td><td>{[c.columnKey, c.extra, c.generated ? 'generated' : ''].filter(Boolean).join(' · ') || '—'}</td><td className="db-actions-cell"><div className="db-row-actions"><button className="icon-button" title={`Edit column ${c.name}`} aria-label={`Edit column ${c.name}`} disabled={blocked || !writable || sqlite || !editableColumn(c)} onClick={() => begin('change_column', undefined, c)}><Pencil size={14} /></button><button className="icon-button db-icon-danger" title={`Delete column ${c.name}`} aria-label={`Delete column ${c.name}`} disabled={blocked || !writable || (sqlite && c.columnKey === 'PRI')} onClick={() => begin('drop_column', undefined, c)}><Trash2 size={14} /></button></div></td></tr>)}</tbody></table></div>
            <p className="db-hint">{sqlite ? 'Use SQL for foreign-key changes, column renames and type changes. Column deletion requires SQLite 3.35 or newer; dependent constraints may prevent it.' : 'Foreign-key changes, engine changes, and advanced column definitions are available through SQL or phpMyAdmin.'}</p>
            {structure && <SchemaDetails structure={structure} database={database} sqlite={sqlite} writable={!!writable} blocked={blocked} canCreate={indexableColumns(page.columns, sqlite).length > 0} onCreate={() => begin('create_index')} onRemove={index => begin('drop_index', undefined, undefined, index)} onForeignKeys={() => setForeignKeyExamples('structure')} />}
          </> : pane === 'query' ? <div className="db-query">
            <div className="db-query-heading"><label htmlFor="db-sql">SQL for <strong>{sqlite ? fileName(database) : database}</strong></label><HelpTip label="SQL query help"><p>SQL changes run immediately. Cancel cannot undo completed statements.</p><p>Results show up to 200 rows per statement.{!sqlite && ' SELECT without a LIMIT is capped at 201.'} Use LIMIT and OFFSET to see more.</p><p>{sqlite ? 'Run up to ten statements and 1 MiB of SQL at once. Finish transactions within the same run. SQL changes do not create automatic backups.' : 'All statements run; only the first ten result sets are displayed. Finish transactions with COMMIT or ROLLBACK in the same run. Import full SQL dumps through Database actions → Import SQL file.'}</p></HelpTip></div><textarea ref={editor} id="db-sql" value={sql} disabled={blocked} onChange={e => setSql(e.target.value)} rows={8} spellCheck={false} onKeyDown={e => { if ((e.metaKey || e.ctrlKey) && e.key === 'Enter' && !blocked && sql.trim()) { e.preventDefault(); void runQuery(); } }} />
            <div className="db-row-tools"><button className="primary" disabled={blocked || !sql.trim()} onClick={() => void runQuery()}>Run selection / all</button><span className="db-hint">⌘ / Ctrl + Enter</span>{history.length > 0 && <select aria-label="Query history" value="" disabled={blocked} onChange={e => setSql(e.target.value)}><option value="">Recent queries (this session)</option>{history.map((text, i) => <option key={i} value={text}>{text.slice(0, 70)}</option>)}</select>}</div>
            {query && <div aria-label="Query results"><p className="db-hint">Completed in {query.elapsedMs} ms</p>{query.resultsTruncated && <p className="db-hint" role="status">All statements ran; only the first ten result sets are shown.</p>}{query.sets.map((set, index) => <section key={index} className="db-query-result"><h3>{query.sets.length === 1 ? set.summary : <>Result {index + 1} <span>{set.summary}</span></>}</h3>{set.truncated && <p className="db-hint">Showing the first 200 rows. Use a LIMIT and OFFSET to see more.</p>}{set.columns.length > 0 && <div className="db-grid-scroll" tabIndex={0} role="region" aria-label={`Query result ${index + 1}`}><table className="db-grid"><thead><tr>{set.columns.map((column, i) => <th key={i}>{column}</th>)}</tr></thead><tbody>{set.rows.map((row, i) => <tr key={i}>{row.map((value, j) => <td key={j}><span className="db-cell">{cell(value)}</span></td>)}</tr>)}</tbody></table></div>}</section>)}</div>}
          </div> : null}
        </section>
      }</TableWorkspace>}

    </div>}
    <footer className="db-footer" inert={browsing}>{available && sqlite && <p className="db-hint">Backups are SQLite database files. Opening a file does not copy or import it into Fairway.</p>}<button className="secondary" disabled={blocked} onClick={() => open('backups')}><FolderOpen size={14} />Open backups</button></footer>
    {pending && showPending && <div className="db-pending" role="status"><LoaderCircle className="spin" size={14} />{pending}{['Running SQL…', 'Loading rows…', 'Sorting rows…'].includes(pending) && <button className="secondary" disabled={!job.current} onClick={() => { if (job.current) void invoke('cancel_data_browser', { id: job.current }).catch(error => setNotice({ error: true, text: errorText(error) })); }}>Cancel query</button>}</div>}
    {viewedRow && <RowViewer table={viewedRow.table} columns={viewedRow.columns} row={viewedRow.row} position={viewedRow.position} largeFields={viewedRow.largeFields} readCell={cellReader(viewedRow.database, viewedRow.table, viewedRow.source, viewedRow.rowRef)} onClose={() => setViewedRow(null)} onEdit={canEdit && !blocked && viewedRow.database === database && viewedRow.table === table && viewedRow.source === source ? () => { setViewedRow(null); begin('update', viewedRow.row, undefined, undefined, viewedRow.rowRef, viewedRow.largeFields); } : undefined} />}
    {edit && <Dialog key={edit.kind} title={titles[edit.kind]} busy={blocked} onClose={() => setEdit(null)} onSubmit={() => { if (edit.kind === 'import_sql') { void importSql(); return; } void save(); }} onCancelBusy={edit.kind === 'import_sql' && (importPhase === 'working' || importPhase === 'backup' || importPhase === 'import') ? cancelImport : undefined} submit={edit.kind === 'import_sql' ? 'Import SQL' : edit.kind === 'drop_index' ? 'Remove' : destructive ? 'Delete' : edit.kind.startsWith('create_') ? 'Create' : 'Save'} danger={destructive} valid={!fieldLoading && (edit.kind === 'import_sql' ? confirmation === database : (edit.kind === 'create_index' ? !!indexDefinition.name.trim() && indexDefinition.columns.length > 0 && indexDefinition.columns.every(Boolean) : !destructive || edit.kind === 'delete' || confirmation === target))}>
      {notice?.error && <div className="notice error" role="alert"><span>{notice.text}</span></div>}
      {edit.kind === 'import_sql' && <>
        <p>Import <strong>{fileName(edit.path ?? '')}</strong> into <strong>{database}</strong>.</p>
        <p className="db-import-path">{edit.path}</p>
        <p>Existing data may be replaced. Fairway backs up <strong>{database}</strong> first. SQL can also change other databases; only this target is backed up automatically. Errors or cancellation can leave partial changes.</p>
        {importPhase && <p role="status">{importPhase === 'working' ? 'Backing up, then importing SQL…' : importPhase === 'backup' ? 'Creating a backup…' : importPhase === 'import' ? 'Importing SQL…' : importPhase === 'canceling' ? 'Canceling import… Waiting for the current step to stop.' : 'Refreshing the database…'}</p>}
        <label>Type {database} to confirm<input autoComplete="off" value={confirmation} onChange={event => setConfirmation(event.target.value)} required /></label>
      </>}
      {needsName && <label>{edit.kind.includes('database') ? 'Database name' : 'Table name'}<input required maxLength={64} value={name} onChange={e => setName(e.target.value)} /></label>}
      {edit.kind === 'rename_database' && <p>Only empty databases can be renamed. For a populated database, export it, create a new database, and import it into the new database. Update your project’s connection settings afterward.</p>}
      {edit.kind === 'create_table' && <div className="db-new-columns"><div className="db-actions"><button type="button" className="secondary" onClick={() => setForeignKeyExamples('create')}><Link2 size={14} />Foreign keys</button></div>{definitions.map((definition, index) => <section className="db-column-card" key={index}><h3>Column {index + 1}</h3><ColumnFields sqlite={sqlite} value={definition} onChange={value => setDefinitions(previous => previous.map((c, i) => i === index ? value : c))} /><button type="button" className="db-danger-text" disabled={definitions.length === 1} onClick={() => setDefinitions(previous => previous.filter((_, i) => i !== index))}>Remove column</button></section>)}<button type="button" className="secondary" onClick={() => setDefinitions(previous => [...previous, { ...newColumn('', sqlite), ...(sqlite ? { type: 'TEXT' } : {}) }])} disabled={definitions.length >= 100}><Plus size={14} />Add column</button></div>}
      {columnForm && <><ColumnFields sqlite={sqlite} adding={edit.kind === 'add_column'} value={definitions[0]} existing={edit.kind === 'change_column'} hasPrimary={edit.kind === 'add_column' && (sqlite || !!page?.columns.some(c => c.columnKey === 'PRI'))} onChange={value => setDefinitions([value])} />{edit.kind === 'change_column' && <p>Changing type or length can alter existing values. A database backup is saved first. Primary keys are preserved; manage indexes in SQL.</p>}</>}
      {(edit.kind === 'insert' || edit.kind === 'update') && page && <RowFields columns={page.columns} original={edit.row} largeFields={edit.largeFields} readCell={cellReader(database, table, source, edit.rowRef)} onLoadingChange={setFieldLoading} onChange={setValues} />}
      {edit.kind === 'create_index' && page && <IndexFields value={indexDefinition} table={table} columns={indexableColumns(page.columns, sqlite)} onChange={setIndexDefinition} />}
      {destructive && <><p>{edit.kind === 'delete' ? 'Delete this row permanently? The original values will be checked before deletion.' : edit.kind === 'drop_index' ? `Remove index “${target}”? Rows are kept.${edit.index?.unique ? ' This stops enforcing uniqueness through this index.' : ''} A ${sqlite ? 'SQLite' : 'SQL'} backup of this database will be saved first.` : `Delete “${target}” permanently? A ${sqlite ? 'SQLite' : 'SQL'} backup of this database will be saved first. Related application code may need changes.`}</p>{edit.kind !== 'delete' && <label>Type {target} to confirm<input autoComplete="off" value={confirmation} onChange={e => setConfirmation(e.target.value)} required /></label>}</>}
    </Dialog>}
    {tableSql && <Dialog title="Table SQL" busy={false} onClose={() => setTableSql(null)} onSubmit={() => {}} showSubmit={false} cancel="Close"><SqlBlock title={tableSql.table} sql={tableSql.sql} copyLabel="Copy table SQL" /></Dialog>}
    {foreignKeyExamples && <ForeignKeyExamples sqlite={sqlite} context={foreignKeyExamples} onClose={() => setForeignKeyExamples(null)} />}
  </Workspace>;
}
