export type Row = Record<string, string | null>;
export type Column = {
  name: string; type: string; nullable: boolean; defaultValue: string | null;
  extra: string; columnKey: string; expression: string; binary: boolean; generated: boolean;
};
export type Table = { name: string; kind: string };
export type Structure = { columns: Column[]; key: string[]; kind: string; sql?: string };
export type Page = Structure & { rows: Row[]; total: string; offset: number; limit: number };
export type QueryResult = { sets: { columns: string[]; rows: (string | null)[][]; affected: number; truncated: boolean }[]; elapsedMs: number };
export type Definition = {
  name: string; type: string; length: string; scale: string; unsigned: boolean;
  nullable: boolean; primary: boolean; autoIncrement: boolean;
  defaultMode: 'none' | 'null' | 'value' | 'current_timestamp'; defaultValue: string;
};
export const columnTypes = ['INT', 'BIGINT', 'SMALLINT', 'TINYINT', 'DECIMAL', 'FLOAT', 'DOUBLE', 'CHAR', 'VARCHAR', 'TEXT', 'LONGTEXT', 'BOOLEAN', 'DATE', 'DATETIME', 'TIMESTAMP', 'JSON', 'BLOB'];
export function newColumn(name = ''): Definition {
  return { name, type: 'VARCHAR', length: '255', scale: '2', unsigned: false, nullable: true, primary: false, autoIncrement: false, defaultMode: 'none', defaultValue: '' };
}
export function fromColumn(c: Column): Definition {
  const parts = /^(\w+)(?:\((\d+)(?:,(\d+))?\))?/i.exec(c.type);
  const current = /^current_timestamp(?:\(\d*\))?$/i.test(c.defaultValue ?? '');
  return { ...newColumn(c.name), type: parts?.[1].toUpperCase() ?? c.type, length: parts?.[2] ?? '255', scale: parts?.[3] ?? '2', unsigned: /unsigned/i.test(c.type), nullable: c.nullable, primary: c.columnKey === 'PRI', autoIncrement: /auto_increment/i.test(c.extra), defaultMode: current ? 'current_timestamp' : c.defaultValue === null ? (c.nullable ? 'null' : 'none') : 'value', defaultValue: c.defaultValue ?? '' };
}
// Preserve advanced definitions by directing their edits to SQL instead of silently losing attributes.
export function editableColumn(c: Column): boolean {
  return !c.generated && columnTypes.includes(fromColumn(c).type) && !/on update|invisible|zerofill/i.test(`${c.type} ${c.extra}`) && (!/DEFAULT_GENERATED/i.test(c.extra) || fromColumn(c).defaultMode === 'current_timestamp') && !/\(\d+\)/.test(c.type.replace(/^(?:varchar|char|decimal)\([^)]*\)/i, ''));
}
