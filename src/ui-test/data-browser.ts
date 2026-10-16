import type { Column, Definition, Row } from '../data/types';

type FixtureTable = { columns: Column[]; rows: Row[]; kind: string };
let schemas: Record<string, Record<string, FixtureTable>>;
const jobs = new Map<string, () => void>();
function column(c: Definition): Column {
  return { name: c.name, type: c.type.toLowerCase() + (['CHAR', 'VARCHAR', 'DECIMAL'].includes(c.type) ? `(${c.length}${c.type === 'DECIMAL' ? `,${c.scale}` : ''})` : '') + (c.unsigned ? ' unsigned' : ''), nullable: c.nullable && !c.primary, defaultValue: c.defaultMode === 'null' || c.defaultMode === 'none' ? null : c.defaultMode === 'current_timestamp' ? 'CURRENT_TIMESTAMP' : c.defaultValue, extra: c.autoIncrement ? 'auto_increment' : '', columnKey: c.primary ? 'PRI' : '', expression: '', binary: c.type === 'BLOB', generated: false };
}
export function resetData() {
  const base: Column = { name: 'id', type: 'bigint unsigned', nullable: false, defaultValue: null, extra: 'auto_increment', columnKey: 'PRI', expression: '', binary: false, generated: false };
  const students: FixtureTable = { kind: 'BASE TABLE', columns: [base, { ...base, name: 'name', type: 'varchar(255)', extra: '', columnKey: '' }, { ...base, name: 'notes', type: 'text', nullable: true, extra: '', columnKey: '' }], rows: [{ id: '9007199254740993', name: 'Student May', notes: null }, ...Array.from({ length: 54 }, (_, i) => ({ id: String(i + 1), name: `Student ${i + 1}`, notes: i === 0 ? '' : 'Learning PHP' }))] };
  schemas = { fairway: { students, messages_without_key: { kind: 'BASE TABLE', columns: [{ ...base, name: 'message', type: 'text', extra: '', columnKey: '' }], rows: [{ message: 'Browse and insert only' }] }, student_names: { ...structuredClone(students), kind: 'VIEW' } }, school: {} };
}
resetData();
export function cancelData(id: string) { jobs.get(id)?.(); }
export async function databaseRequest(id: string, r: Record<string, unknown>) {
  await new Promise<void>((resolve, reject) => {
    const timer = window.setTimeout(() => { jobs.delete(id); resolve(); }, String(r.sql ?? '').toLowerCase().includes('sleep(') ? 15000 : 60);
    jobs.set(id, () => { clearTimeout(timer); jobs.delete(id); reject(new Error('Database request canceled or timed out. Completed SQL statements are not undone; refresh to check the current data.')); });
  });
  const op = String(r.op), db = String(r.database), name = String(r.name), table = String(r.table);
  if (op === 'databases') return { databases: Object.keys(schemas).sort() };
  if (op === 'create_database') { if (schemas[db]) throw new Error('Database already exists.'); schemas[db] = {}; return { message: 'Database created.' }; }
  const schema = schemas[db]; if (!schema) throw new Error('Unknown database. Refresh the database list.');
  if (op === 'drop_database') { delete schemas[db]; return { message: 'Database deleted. A SQL backup was saved first.' }; }
  if (op === 'rename_database') { if (Object.keys(schema).length) throw new Error('Only empty databases can be renamed. Use SQL export/import for a populated database.'); if (schemas[name]) throw new Error('Database already exists.'); schemas[name] = schema; delete schemas[db]; return { message: 'Database renamed. Update project connection settings.' }; }
  if (op === 'tables') return { tables: Object.entries(schema).map(([name, t]) => ({ name, kind: t.kind })) };
  if (op === 'export') return { message: 'SQL export saved: /Users/Student May/Library/Application Support/dev.fairway.environment/backups/database-ui-test.sql' };
  if (op === 'query') {
    const sql = String(r.sql).trim();
    if (!/^select\s/i.test(sql)) throw new Error('Simulated SQL error. Use SELECT VERSION() or SELECT * FROM students for visual review. Native SQL is verified separately.');
    const selected = /from\s+`?students`?/i.test(sql) ? schema.students : null;
    return { elapsedMs: 12, sets: [{ columns: selected ? selected.columns.map(c => c.name) : ['VERSION()'], rows: selected ? selected.rows.map(row => selected.columns.map(c => row[c.name])) : [['9.7.0 (UI fixture)']], affected: 0, truncated: false }] };
  }
  if (op === 'create_table') { if (schema[table]) throw new Error('Table already exists.'); schema[table] = { kind: 'BASE TABLE', columns: (r.columns as Definition[]).map(column), rows: [] }; return { message: 'Table created.' }; }
  const target = schema[table]; if (!target) throw new Error('This table no longer exists.');
  const key = target.kind === 'VIEW' ? [] : target.columns.filter(c => c.columnKey === 'PRI').map(c => c.name);
  if (op === 'structure') return { columns: target.columns, key, kind: target.kind, sql: `CREATE TABLE \`${table}\` (\n${target.columns.map(c => `  \`${c.name}\` ${c.type}`).join(',\n')}\n)` };
  if (op === 'rows') {
    const filtered = target.rows.filter(row => !r.filterColumn || !r.filterValue || (row[String(r.filterColumn)] ?? '').includes(String(r.filterValue)));
    if (r.sort) filtered.sort((a, b) => String(a[String(r.sort)]).localeCompare(String(b[String(r.sort)]), undefined, { numeric: true }) * (r.direction === 'DESC' ? -1 : 1));
    const offset = Number(r.offset ?? 0);
    return { columns: target.columns, key, kind: target.kind, rows: filtered.slice(offset, offset + 50), total: String(filtered.length), offset, limit: 50 };
  }
  if (op === 'rename_table') { if (schema[name]) throw new Error('Table already exists.'); schema[name] = target; delete schema[table]; }
  if (op === 'drop_table') delete schema[table];
  if (op === 'insert') {
    const row: Row = {}; const supplied = r.values as Row;
    for (const c of target.columns) row[c.name] = Object.hasOwn(supplied, c.name) ? supplied[c.name] : c.extra === 'auto_increment' ? String(target.rows.length + 100) : c.defaultValue;
    if (target.columns.some(c => !c.nullable && row[c.name] === null)) throw new Error('A required column has no value or default.');
    target.rows.push(row);
  }
  if (op === 'update' || op === 'delete') {
    if (!key.length) throw new Error('This table has no safe row identity.');
    const index = target.rows.findIndex(row => JSON.stringify(row) === JSON.stringify(r.original));
    if (index < 0) throw new Error('This row changed or was deleted. Refresh before saving.');
    if (op === 'delete') target.rows.splice(index, 1); else Object.assign(target.rows[index], r.values);
  }
  if (op === 'add_column' || op === 'change_column') {
    const c = column(r.column as Definition); const index = target.columns.findIndex(old => old.name === r.oldName);
    if (op === 'change_column') { target.columns[index] = c; for (const row of target.rows) { row[c.name] = row[String(r.oldName)]; if (c.name !== r.oldName) delete row[String(r.oldName)]; } }
    else { target.columns.push(c); for (const row of target.rows) row[c.name] = c.defaultValue; }
  }
  if (op === 'drop_column') { target.columns = target.columns.filter(c => c.name !== name); for (const row of target.rows) delete row[name]; }
  return { message: op.startsWith('drop_') ? 'Deleted. A SQL backup was saved first.' : 'Changes saved.' };
}
