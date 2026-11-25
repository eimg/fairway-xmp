import { useState } from 'react';
import Dialog from '../data/Dialog';
import type { PgOutput } from './explorer-types';
export default function PgResult({ output, title }: { output: PgOutput; title: string }) {
  const [cell, setCell] = useState<{ label: string; value: string | null } | null>(null);
  return <section className="pg-result" aria-label={title}>
    <div className="pg-result-summary"><strong>{output.command || 'Result'}</strong><span>{output.rows.length} rows returned{output.affectedRows !== null ? ` · ${output.affectedRows} affected` : ''}</span><span>{output.outcome === 'committed' ? 'Committed' : output.outcome === 'unknown' ? 'Outcome unknown' : 'Not committed'}</span></div>
    {output.truncated && <p className="pg-warning" role="status">Result truncated. More rows may exist; the statement was cancelled at its result limit.</p>}
    {output.warnings.map((warning, index) => <p key={index} className="db-hint">{warning}</p>)}
    {output.columns.length > 0 ? <div className="db-grid-scroll pg-grid-scroll" role="region" aria-label={`${title} table`} tabIndex={0}><table className="db-grid"><thead><tr>{output.columns.map((column, index) => <th key={index} scope="col"><span>{column.name}</span><small title={`OID ${column.typeOid}; modifier ${column.typeModifier}`}>{column.typeName}</small></th>)}</tr></thead><tbody>{output.rows.map((row, ri) => <tr key={ri}>{output.columns.map((column, ci) => <td key={ci}><button className="pg-cell" aria-label={`View ${column.name}, column ${ci + 1}, row ${ri + 1}`} onClick={() => setCell({ label: `${column.name} · row ${ri + 1}`, value: row[ci] })}>{row[ci] === null ? <em>NULL</em> : row[ci] === '' ? <em>Empty string</em> : row[ci]}</button></td>)}</tr>)}</tbody></table>{!output.rows.length && <p className="db-grid-empty">No rows returned.</p>}</div> : <p className="db-hint">No row data returned. Check the outcome before rerunning a modifying statement.</p>}
    {cell && <Dialog title="PostgreSQL value" busy={false} onClose={() => setCell(null)} onSubmit={() => {}} showSubmit={false} cancel="Close"><p>{cell.label}</p><p className="db-hint">Exact returned text. NULL and empty strings remain distinct.</p>{cell.value === null ? <p>NULL</p> : <textarea aria-label="Exact PostgreSQL value" readOnly value={cell.value} rows={10} spellCheck={false} />}</Dialog>}
  </section>;
}
