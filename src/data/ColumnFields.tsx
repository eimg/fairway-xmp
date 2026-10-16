import { columnTypes, type Definition } from './types';

export default function ColumnFields({ value: c, onChange, existing = false, hasPrimary = false }: {
  value: Definition; onChange: (value: Definition) => void; existing?: boolean; hasPrimary?: boolean;
}) {
  function set<K extends keyof Definition>(key: K, value: Definition[K]) { onChange({ ...c, [key]: value }); }
  const integer = ['INT', 'BIGINT', 'SMALLINT', 'TINYINT'].includes(c.type);
  return <div className="db-column-fields">
    <label>Column name<input required value={c.name} maxLength={64} onChange={e => set('name', e.target.value)} /></label>
    <label>Type<select value={c.type} onChange={e => onChange({ ...c, type: e.target.value, autoIncrement: false, length: e.target.value === 'DECIMAL' ? '10' : '255', defaultMode: 'none' })}>{columnTypes.map(type => <option key={type}>{type}</option>)}</select></label>
    {['VARCHAR', 'CHAR', 'DECIMAL'].includes(c.type) && <label>{c.type === 'DECIMAL' ? 'Precision' : 'Length'}<input required type="number" min="1" max={c.type === 'DECIMAL' ? 65 : c.type === 'CHAR' ? 255 : 16383} value={c.length} onChange={e => set('length', e.target.value)} /></label>}
    {c.type === 'DECIMAL' && <label>Scale<input required type="number" min="0" max={Math.min(30, Number(c.length))} value={c.scale} onChange={e => set('scale', e.target.value)} /></label>}
    <div className="db-checks">
      <label><input type="checkbox" checked={c.nullable} disabled={c.primary || c.autoIncrement} onChange={e => onChange({ ...c, nullable: e.target.checked, defaultMode: c.defaultMode === 'null' ? 'none' : c.defaultMode })} />Allow NULL</label>
      <label><input type="checkbox" checked={c.primary} disabled={existing || hasPrimary} onChange={e => onChange({ ...c, primary: e.target.checked, nullable: false, autoIncrement: false, defaultMode: 'none' })} />Primary key</label>
      <label><input type="checkbox" checked={c.autoIncrement} disabled={!integer || !c.primary} onChange={e => onChange({ ...c, autoIncrement: e.target.checked, nullable: false, defaultMode: 'none' })} />Auto-increment</label>
      {(integer || ['DECIMAL', 'FLOAT', 'DOUBLE'].includes(c.type)) && <label><input type="checkbox" checked={c.unsigned} onChange={e => set('unsigned', e.target.checked)} />Unsigned</label>}
    </div>
    <label>Default<select value={c.defaultMode} disabled={c.autoIncrement} onChange={e => set('defaultMode', e.target.value as Definition['defaultMode'])}><option value="none">No default</option>{c.nullable && <option value="null">NULL</option>}<option value="value">Literal value</option>{['DATETIME', 'TIMESTAMP'].includes(c.type) && <option value="current_timestamp">CURRENT_TIMESTAMP</option>}</select></label>
    {c.defaultMode === 'value' && <label>Default value<input value={c.defaultValue} onChange={e => set('defaultValue', e.target.value)} placeholder="Empty text is an empty string" /></label>}
  </div>;
}
