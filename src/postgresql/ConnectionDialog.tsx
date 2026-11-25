import { useRef, useState } from 'react';
import Dialog from '../data/Dialog';
import type { ConnectRequest, Connection } from './workspace-model';
import type { ManagedPostgresSnapshot } from './types';
export default function ConnectionDialog({ managed, initial, database, busy, onClose, onConnect }: { managed: ManagedPostgresSnapshot | null; initial?: Connection; database?: string; busy: boolean; onClose: () => void; onConnect: (request: ConnectRequest) => void }) {
  const [kind, setKind] = useState(initial?.source.kind ?? 'managed');
  const [db, setDb] = useState(database ?? initial?.session.database ?? 'postgres');
  const [host, setHost] = useState(initial?.source.kind === 'external' ? initial.source.host : '127.0.0.1');
  const [port, setPort] = useState(initial?.source.kind === 'external' ? String(initial.source.port) : '5432');
  const [user, setUser] = useState(initial?.source.kind === 'external' ? initial.source.user : '');
  const [consent, setConsent] = useState(false);
  const password = useRef<HTMLInputElement>(null);
  const valid = !!db.trim() && new TextEncoder().encode(db).length <= 63 && consent && (kind === 'managed' ? !!managed?.clusterId && !!managed.running : !!user.trim() && Number.isInteger(Number(port)) && Number(port) > 0 && Number(port) <= 65535);
  return <Dialog title="Connect to PostgreSQL" submit="Connect" busy={busy} valid={valid} onClose={onClose} onSubmit={() => {
    if (!valid) return;
    const secret = password.current?.value ?? ''; if (password.current) password.current.value = '';
    const target = kind === 'managed' ? { kind: 'managed' as const, clusterId: managed!.clusterId! } : { kind: 'external' as const, host, port: Number(port), user, password: secret, transport: 'loopback' as const };
    onConnect({ confirmed: true, database: db, target });
  }}>
    <label>Connection source<select aria-label="Connection source" value={kind} onChange={e => { setKind(e.target.value as 'managed' | 'external'); setConsent(false); if (password.current) password.current.value = ''; }}><option value="managed">Fairway managed PostgreSQL</option><option value="external">External PostgreSQL · loopback</option></select></label>
    <label>Database<input aria-label="Connection database" value={db} onChange={e => setDb(e.target.value)} required maxLength={63} /></label>
    {kind === 'managed' ? <p className="db-hint">Uses this Fairway cluster's Keychain credential. {managed?.running ? `Process running on port ${managed.port}; authenticated access is checked when connecting.` : 'Initialize and explicitly start a managed cluster first.'} No password fallback is available.</p> : <>
      <p className="pg-warning">Externally owned server. Fairway only opens database sessions; it will not start, stop, initialize or reconfigure this installation.</p>
      <label>Host<select aria-label="PostgreSQL host" value={host} onChange={e => setHost(e.target.value)}><option>127.0.0.1</option><option>::1</option></select></label>
      <div className="pg-form-pair"><label>Port<input aria-label="PostgreSQL port" type="number" min={1} max={65535} value={port} onChange={e => setPort(e.target.value)} /></label><label>User<input aria-label="PostgreSQL user" value={user} onChange={e => setUser(e.target.value)} autoComplete="off" maxLength={63} required /></label></div>
      <label>Password<input ref={password} aria-label="PostgreSQL password" type="password" autoComplete="off" maxLength={4096} /></label>
      <p className="db-hint">Session only: cleared from this form on submission, never saved in history or settings. Disconnect releases the native credential. Re-enter it to open another database.</p>
      <p className="db-hint">Loopback without TLS is an explicit local choice. Remote connections are unavailable until verify-full TLS is supported. External PostgreSQL 17.x or 18.6 and the verified Fairway 18.6 client runtime are required.</p>
    </>}
    <label className="pg-check"><input type="checkbox" checked={consent} onChange={e => setConsent(e.target.checked)} />Authorize this database connection</label>
  </Dialog>;
}
