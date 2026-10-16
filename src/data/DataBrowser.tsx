import { useEffect, useRef, useState } from 'react';
import { ArrowUpRight, ChevronLeft, ChevronRight, Database, LoaderCircle, Plus, RefreshCw } from 'lucide-react';
import { invoke } from '../bridge';
import Dialog from './Dialog';
import ColumnFields from './ColumnFields';
import RowFields, { initialInsertValues } from './RowFields';
import TableWorkspace from './TableWorkspace';
import ActionMenu from './ActionMenu';
import HelpTip from '../HelpTip';
import { editableColumn, fromColumn, newColumn, type Column, type Definition, type Page, type QueryResult, type Row, type Structure, type Table } from './types';
import './data.css';

type EditKind = 'create_database' | 'rename_database' | 'drop_database' | 'create_table' | 'rename_table' | 'drop_table' | 'add_column' | 'change_column' | 'drop_column' | 'insert' | 'update' | 'delete';
type Edit = { kind: EditKind; row?: Row; column?: Column };
const titles: Record<EditKind, string> = { create_database: 'Create database', rename_database: 'Rename database', drop_database: 'Delete database', create_table: 'Create table', rename_table: 'Rename table', drop_table: 'Delete table', add_column: 'Add column', change_column: 'Edit column', drop_column: 'Delete column', insert: 'Insert row', update: 'Edit row', delete: 'Delete row' };
const errorText = (error: unknown) => error instanceof Error ? error.message : String(error);
const idColumn = (): Definition => ({ ...newColumn('id'), type: 'BIGINT', nullable: false, primary: true, autoIncrement: true });
const cell = (value: string | null) => value === null ? <span className="db-null">NULL</span> : value === '' ? <span className="db-null">empty</span> : value;

export default function DataBrowser({ ready, busy: appBusy, start, open }: { ready: boolean; busy: boolean; start: () => void; open: (target: string) => void }) {
  const [databases, setDatabases] = useState<string[]>([]);
  const [catalogLoaded, setCatalogLoaded] = useState(false);
  const [database, setDatabase] = useState('');
  const [tables, setTables] = useState<Table[]>([]);
  const [table, setTable] = useState('');
  const [page, setPage] = useState<Page | null>(null);
  const [structure, setStructure] = useState<Structure | null>(null);
  const [pane, setPane] = useState<'rows' | 'structure' | 'query'>('rows');
  const [pending, setPending] = useState<string | null>(null);
  const [notice, setNotice] = useState<{ error: boolean; text: string } | null>(null);
  const [edit, setEdit] = useState<Edit | null>(null);
  const [name, setName] = useState('');
  const [confirmation, setConfirmation] = useState('');
  const [definitions, setDefinitions] = useState<Definition[]>([]);
  const [values, setValues] = useState<Row>({});
  const [sort, setSort] = useState({ sort: '', direction: 'ASC' });
  const [sql, setSql] = useState('SELECT VERSION();');
  const [query, setQuery] = useState<QueryResult | null>(null);
  const [history, setHistory] = useState<string[]>([]);
  const editor = useRef<HTMLTextAreaElement>(null);
  const active = useRef(false);
  const activeTask = useRef<Promise<void> | null>(null);
  const job = useRef<string | null>(null);
  const mounted = useRef(false);
  const browsing = !!pending && /^(Loading|Refreshing|Sorting) /.test(pending);
  const [showPending, setShowPending] = useState(false);
  const blocked = (!!pending && !browsing) || appBusy;
  const writable = page?.kind === 'BASE TABLE';
  const canEdit = writable && !!page?.key.length;

  async function request<T>(body: Record<string, unknown>): Promise<T> {
    const id = crypto.randomUUID(); job.current = id;
    try { return await invoke<T>('data_browser', { id, request: body }); }
    finally { if (job.current === id) job.current = null; }
  }
  async function run(label: string, action: () => Promise<void>) {
    if (active.current) return;
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
  async function load(preferredDb = database, preferredTable = table) {
    const list = await request<{ databases: string[] }>({ op: 'databases' });
    const db = list.databases.includes(preferredDb) ? preferredDb : (list.databases[0] ?? '');
    const result = db ? await request<{ tables: Table[] }>({ op: 'tables', database: db }) : { tables: [] };
    const selected = result.tables.find(t => t.name === preferredTable)?.name ?? result.tables[0]?.name ?? '';
    const nextPage = selected ? await request<Page>({ op: 'rows', database: db, table: selected, offset: 0 }) : null;
    const nextStructure = selected ? await request<Structure>({ op: 'structure', database: db, table: selected }) : null;
    // Publish a complete catalog together: never pair a new database/table with old rows.
    if (db !== database) setQuery(null);
    setDatabases(list.databases); setDatabase(db); setTables(result.tables);
    setTable(selected); setPage(nextPage); setStructure(nextStructure);
    setCatalogLoaded(true);
    setSort({ sort: '', direction: 'ASC' });
  }
  useEffect(() => {
    setShowPending(false);
    if (!pending) return;
    if (!browsing) { setShowPending(true); return; }
    const timer = window.setTimeout(() => setShowPending(true), 200);
    return () => window.clearTimeout(timer);
  }, [pending, browsing]);
  useEffect(() => {
    let disposed = false;
    // A sidebar return may overlap cancellation of the previous view's request.
    // Wait for it before refreshing, so an initial load cannot be skipped forever.
    void (async () => {
      await activeTask.current;
      if (disposed) return;
      if (ready) void run('Loading databases…', () => load());
      else { setPage(null); setStructure(null); }
    })();
    return () => { disposed = true; };
    // Refresh on visibility/service changes, not on each snapshot poll.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [ready]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      // React's development effect replay is not an actual navigation away.
      queueMicrotask(() => { if (!mounted.current && job.current) void invoke('cancel_data_browser', { id: job.current }).catch(() => {}); });
    };
  }, []);

  function begin(kind: EditKind, row?: Row, column?: Column) {
    setNotice(null); setName(kind === 'rename_database' ? database : kind === 'rename_table' ? table : '');
    setConfirmation(''); setValues(kind === 'insert' && page ? initialInsertValues(page.columns) : {});
    setDefinitions(kind === 'create_table' ? [idColumn(), newColumn('name')] : column ? [fromColumn(column)] : [newColumn()]);
    setEdit({ kind, row, column });
  }
  async function save() {
    if (!edit) return;
    const current = edit;
    await run('Saving database changes…', async () => {
      const body: Record<string, unknown> = { op: current.kind, database, table, confirmed: true };
      if (current.kind === 'create_database') body.database = name;
      if (['rename_database', 'rename_table', 'drop_column'].includes(current.kind)) body.name = current.kind === 'drop_column' ? current.column?.name : name;
      if (current.kind === 'create_table') { body.table = name; body.columns = definitions; }
      if (current.kind === 'add_column' || current.kind === 'change_column') { body.column = definitions[0]; body.oldName = current.column?.name; }
      if (current.row) body.original = current.row;
      if (current.kind === 'insert' || current.kind === 'update') body.values = values;
      const result = await request<{ message: string }>(body);
      setEdit(null);
      await load(current.kind === 'create_database' || current.kind === 'rename_database' ? name : database, current.kind === 'create_table' || current.kind === 'rename_table' ? name : table);
      setNotice({ error: false, text: result.message });
    });
  }
  function showPane(next: typeof pane) {
    setPane(next);
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
      setQuery(result); setHistory(previous => [text, ...previous.filter(item => item !== text)].slice(0, 10));
      // SQL can change or remove the selected table. Refresh the catalog after execution.
      await load();
    });
  }
  const destructive = edit?.kind.startsWith('drop_') || edit?.kind === 'delete';
  const target = edit?.kind === 'drop_database' ? database : edit?.kind === 'drop_table' ? table : edit?.column?.name ?? '';
  const needsName = edit && ['create_database', 'rename_database', 'create_table', 'rename_table'].includes(edit.kind);
  const columnForm = edit && ['add_column', 'change_column'].includes(edit.kind);

  return <div className="db-browser" aria-busy={!!pending}>
    <div className="page-heading"><div><h1>Data browser</h1><p>Browse data, shape tables, and run SQL.</p></div><button className="secondary" inert={browsing} disabled={blocked} onClick={() => open('phpmyadmin')}>phpMyAdmin<ArrowUpRight size={14} /></button></div>
    {notice && !edit && <div className={`notice ${notice.error ? 'error' : 'success'}`} role={notice.error ? 'alert' : 'status'}><span>{notice.text}</span><button aria-label="Dismiss database message" onClick={() => setNotice(null)}>×</button></div>}
    {!ready ? <section className="settings-card db-empty"><Database size={28} /><h2>Start MySQL to browse your data</h2><p>Your databases stay on this computer.</p><button className="primary" disabled={blocked} onClick={start}>Start MySQL</button></section> : <div inert={browsing}>
      <section className="db-catalog" aria-label="Database controls">
        <label>Database<select aria-label="Database" value={database} disabled={blocked || !databases.length} onChange={e => void run('Loading database…', () => load(e.target.value, ''))}>{!databases.length && <option value="">No databases</option>}{databases.map(db => <option key={db}>{db}</option>)}</select></label>
        <div className="db-actions"><button className="secondary" disabled={blocked} onClick={() => begin('create_database')}><Plus size={14} />Database</button><button className="icon-button" aria-label="Refresh databases" disabled={blocked} onClick={() => void run('Refreshing databases…', () => load())}><RefreshCw size={15} /></button><ActionMenu label="Database actions" disabled={blocked || !database} actions={[
          { label: 'Export database SQL', onSelect: () => void exportSql() },
          { label: 'Rename database', onSelect: () => begin('rename_database') },
          { label: 'Delete database', danger: true, onSelect: () => begin('drop_database') },
        ]} /></div>
      </section>
      {!catalogLoaded && !notice?.error && <div className="db-loading-placeholder" aria-label="Loading database workspace" />}
      {database && <TableWorkspace sidebar={
        <aside className="db-tables" aria-label="Tables"><div className="db-tables-heading"><h2>Tables <span>{tables.length}</span></h2><button className="icon-button" aria-label="Create table" disabled={blocked} onClick={() => begin('create_table')}><Plus size={16} /></button></div>
          {tables.length ? tables.map(t => <button key={t.name} title={t.name} className={`db-table-choice ${table === t.name ? 'selected' : ''}`} disabled={blocked} onClick={() => void run('Loading table…', () => load(database, t.name))}><span>{t.name}</span>{t.kind === 'VIEW' && <small>view</small>}</button>) : <p className="db-hint">No tables yet. Create your first table with +.</p>}
        </aside>
      }>{toggle =>
        <section className="db-detail" aria-label="Database workspace">
          <div className="db-detail-heading"><div className="db-detail-title"><h2>{table || database}</h2></div><div className="db-detail-controls">{toggle}{table && <>{pane === 'rows' && <button className="secondary" disabled={blocked || !writable} onClick={() => begin('insert')}><Plus size={14} />Insert row</button>}<ActionMenu label="Table actions" disabled={blocked} actions={[
            { label: 'Export table SQL', onSelect: () => void exportSql(table) },
            { label: 'Rename table', disabled: !writable, onSelect: () => begin('rename_table') },
            { label: 'Delete table', disabled: !writable, danger: true, onSelect: () => begin('drop_table') },
          ]} /></>}</div></div>
          <div className="db-tabs" role="tablist" aria-label="Data views">{(['rows', 'structure', 'query'] as const).map(tab => <button key={tab} role="tab" aria-selected={pane === tab} disabled={blocked || (tab !== 'query' && !table)} onClick={() => showPane(tab)}>{tab === 'query' ? 'SQL query' : tab === 'rows' ? 'Rows' : 'Structure'}</button>)}</div>
          {!table && pane !== 'query' ? <div className="db-empty"><p>Create a table or open SQL query to begin.</p><button className="secondary" disabled={blocked} onClick={() => showPane('query')}>Open SQL query</button></div> : pane === 'rows' && page ? <>
            {!canEdit && <p className="db-hint">{writable ? 'No non-null primary or unique key: browse and insert are available. Use SQL for row updates and deletes.' : 'This view is read-only. Use SQL for advanced changes.'}</p>}
            <div className="db-grid-scroll" tabIndex={0} role="region" aria-label="Table rows"><table className="db-grid"><thead><tr>{canEdit && <th>Actions</th>}{page.columns.map(c => <th key={c.name}><button disabled={blocked} onClick={() => { const next = { sort: c.name, direction: sort.sort === c.name && sort.direction === 'ASC' ? 'DESC' : 'ASC' }; setSort(next); void run('Sorting rows…', () => loadRows(database, table, 0, next)); }}>{c.name}{sort.sort === c.name ? sort.direction === 'ASC' ? ' ↑' : ' ↓' : ''}</button></th>)}</tr></thead><tbody>{page.rows.map((row, index) => <tr key={index}>{canEdit && <td className="db-row-actions"><button disabled={blocked} onClick={() => begin('update', row)} aria-label={`Edit row ${index + 1}`}>Edit</button><button disabled={blocked} onClick={() => begin('delete', row)} aria-label={`Delete row ${index + 1}`}>Delete</button></td>}{page.columns.map(c => <td key={c.name}><span className="db-cell" title={row[c.name] ?? 'SQL NULL'}>{cell(row[c.name])}</span></td>)}</tr>)}</tbody></table>{!page.rows.length && <p className="db-grid-empty">{writable ? 'No rows yet. Insert a row to get started.' : 'This view has no rows.'}</p>}</div>
            <div className="db-pagination"><span>{page.rows.length ? page.offset + 1 : 0}–{page.offset + page.rows.length} of {page.total}</span><div className="db-actions"><button className="icon-button" aria-label="Previous page" disabled={blocked || page.offset === 0} onClick={() => void run('Loading rows…', () => loadRows(database, table, Math.max(0, page.offset - page.limit)))}><ChevronLeft size={17} /></button><button className="icon-button" aria-label="Next page" disabled={blocked || BigInt(page.offset + page.rows.length) >= BigInt(page.total)} onClick={() => void run('Loading rows…', () => loadRows(database, table, page.offset + page.limit))}><ChevronRight size={17} /></button><button className="icon-button" aria-label="Refresh rows" disabled={blocked} onClick={() => void run('Refreshing rows…', () => loadRows(database, table, page.offset))}><RefreshCw size={14} /></button></div></div>
          </> : pane === 'structure' && page ? <>
            <div className="db-row-tools"><p className="db-hint">{page.key.length ? `Row identity: ${page.key.join(', ')}` : 'No safe row identity found.'}</p><button className="secondary" disabled={blocked || !writable} onClick={() => begin('add_column')}><Plus size={14} />Column</button></div>
            <div className="db-grid-scroll" tabIndex={0} role="region" aria-label="Table structure"><table className="db-grid"><thead><tr><th>Column</th><th>Type</th><th>NULL</th><th>Default</th><th>Attributes</th><th>Actions</th></tr></thead><tbody>{page.columns.map(c => <tr key={c.name}><td>{c.name}</td><td>{c.type}</td><td>{c.nullable ? 'Yes' : 'No'}</td><td>{cell(c.defaultValue)}</td><td>{[c.columnKey, c.extra, c.generated ? 'generated' : ''].filter(Boolean).join(' · ') || '—'}</td><td className="db-row-actions"><button disabled={blocked || !writable || !editableColumn(c)} onClick={() => begin('change_column', undefined, c)} aria-label={`Edit column ${c.name}`}>Edit</button><button disabled={blocked || !writable} onClick={() => begin('drop_column', undefined, c)} aria-label={`Delete column ${c.name}`}>Delete</button></td></tr>)}</tbody></table></div>
            <p className="db-hint">Indexes, foreign keys, engine changes, and advanced column definitions are available through SQL or phpMyAdmin.</p>{structure?.sql && <details className="db-ddl"><summary>Table SQL</summary><pre>{structure.sql}</pre></details>}
          </> : pane === 'query' ? <div className="db-query">
            <div className="db-query-heading"><label htmlFor="db-sql">SQL for <strong>{database}</strong></label><HelpTip label="SQL query help"><p>SQL changes run immediately. Cancel cannot undo completed statements.</p><p>Results show up to 200 rows per statement. SELECT without a LIMIT is capped at 201. Use LIMIT and OFFSET to see more.</p><p>Use the CLI to import full SQL dumps with DELIMITER commands.</p></HelpTip></div><textarea ref={editor} id="db-sql" value={sql} disabled={blocked} onChange={e => setSql(e.target.value)} rows={8} spellCheck={false} onKeyDown={e => { if ((e.metaKey || e.ctrlKey) && e.key === 'Enter' && !blocked && sql.trim()) { e.preventDefault(); void runQuery(); } }} />
            <div className="db-row-tools"><button className="primary" disabled={blocked || !sql.trim()} onClick={() => void runQuery()}>Run selection / all</button><span className="db-hint">⌘ / Ctrl + Enter</span>{history.length > 0 && <select aria-label="Query history" value="" disabled={blocked} onChange={e => setSql(e.target.value)}><option value="">Recent queries (this session)</option>{history.map((text, i) => <option key={i} value={text}>{text.slice(0, 70)}</option>)}</select>}</div>
            {query && <div aria-label="Query results"><p className="db-hint">Completed in {query.elapsedMs} ms</p>{query.sets.map((set, index) => <section key={index} className="db-query-result"><h3>Result {index + 1} <span>{set.columns.length ? `${set.rows.length} rows` : `${set.affected} affected`}</span></h3>{set.truncated && <p className="db-hint">Showing the first 200 rows. Use a LIMIT and OFFSET to see more.</p>}{set.columns.length > 0 && <div className="db-grid-scroll" tabIndex={0} role="region" aria-label={`Query result ${index + 1}`}><table className="db-grid"><thead><tr>{set.columns.map((column, i) => <th key={i}>{column}</th>)}</tr></thead><tbody>{set.rows.map((row, i) => <tr key={i}>{row.map((value, j) => <td key={j}><span className="db-cell">{cell(value)}</span></td>)}</tr>)}</tbody></table></div>}</section>)}</div>}
          </div> : null}
        </section>
      }</TableWorkspace>}
      <details className="db-cli"><summary>Command-line administration</summary><p>Add command-line tools to your user PATH in Settings, then open a new terminal. Exports never overwrite an existing file; imports back up the target database first.</p><pre>fairway db list{'\n'}fairway db export {database || 'my_database'} --output ~/Desktop/backup.sql{'\n'}fairway db import {database || 'my_database'} ~/Desktop/backup.sql --yes{'\n'}fairway db --help</pre><button className="secondary" disabled={blocked} onClick={() => open('backups')}>Open backups</button></details>
    </div>}
    {pending && showPending && <div className="db-pending" role="status"><LoaderCircle className="spin" size={14} />{pending}{['Running SQL…', 'Loading rows…', 'Sorting rows…'].includes(pending) && <button className="secondary" disabled={!job.current} onClick={() => { if (job.current) void invoke('cancel_data_browser', { id: job.current }).catch(error => setNotice({ error: true, text: errorText(error) })); }}>Cancel query</button>}</div>}
    {edit && <Dialog title={titles[edit.kind]} busy={blocked} onClose={() => setEdit(null)} onSubmit={() => void save()} submit={destructive ? 'Delete' : edit.kind.startsWith('create_') ? 'Create' : 'Save'} danger={destructive} valid={!destructive || edit.kind === 'delete' || confirmation === target}>
      {notice?.error && <div className="notice error" role="alert"><span>{notice.text}</span></div>}
      {needsName && <label>{edit.kind.includes('database') ? 'Database name' : 'Table name'}<input required maxLength={64} value={name} onChange={e => setName(e.target.value)} /></label>}
      {edit.kind === 'rename_database' && <p>Only empty databases can be renamed. For a populated database, export it, create a new database, and import with the CLI. Update your project’s connection settings afterward.</p>}
      {edit.kind === 'create_table' && <div className="db-new-columns">{definitions.map((definition, index) => <section className="db-column-card" key={index}><h3>Column {index + 1}</h3><ColumnFields value={definition} onChange={value => setDefinitions(previous => previous.map((c, i) => i === index ? value : c))} /><button type="button" className="db-danger-text" disabled={definitions.length === 1} onClick={() => setDefinitions(previous => previous.filter((_, i) => i !== index))}>Remove column</button></section>)}<button type="button" className="secondary" onClick={() => setDefinitions(previous => [...previous, newColumn()])} disabled={definitions.length >= 100}><Plus size={14} />Add column</button></div>}
      {columnForm && <><ColumnFields value={definitions[0]} existing={edit.kind === 'change_column'} hasPrimary={edit.kind === 'add_column' && !!page?.columns.some(c => c.columnKey === 'PRI')} onChange={value => setDefinitions([value])} />{edit.kind === 'change_column' && <p>Changing type or length can alter existing values. A database backup is saved first. Primary keys are preserved; manage indexes in SQL.</p>}</>}
      {(edit.kind === 'insert' || edit.kind === 'update') && page && <RowFields columns={page.columns} original={edit.row} onChange={setValues} />}
      {destructive && <><p>{edit.kind === 'delete' ? 'Delete this row permanently? The original values will be checked before deletion.' : `Delete “${target}” permanently? A SQL backup of this database will be saved first. Related application code may need changes.`}</p>{edit.kind !== 'delete' && <label>Type {target} to confirm<input autoComplete="off" value={confirmation} onChange={e => setConfirmation(e.target.value)} required /></label>}</>}
    </Dialog>}
  </div>;
}
