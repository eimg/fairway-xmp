import { useCallback, useEffect, useRef, useState } from 'react';
import {
  ArrowDownToLine, ArrowUpRight, Check, ChevronRight, CircleHelp, Clipboard, Code2,
  Database, Download, ExternalLink, FolderOpen, Globe, Leaf, LoaderCircle, Play,
  RefreshCw, ScrollText, Settings2, ShieldCheck, Square, Terminal, X,
} from 'lucide-react';
import { invoke, isDesktop, listen, preview } from './bridge';
import type { Ports, ServiceId, ServiceStatus, Snapshot, UpdateInfo } from './types';

const meta = {
  nginx: { name: 'Nginx', description: 'Your local web server', icon: Globe, version: 'nginx', color: 'green' },
  php: { name: 'PHP', description: 'Managed automatically with Nginx', icon: Code2, version: 'php', color: 'purple' },
  mysql: { name: 'MySQL', description: 'Your local database', icon: Database, version: 'mysql', color: 'blue' },
  phpmyadmin: { name: 'phpMyAdmin', description: 'Manage tables and data', icon: Database, version: 'phpmyadmin', color: 'orange' },
} as const;

type View = 'environment' | 'settings' | 'updates';
type Notice = { text: string; kind: 'error' | 'success' } | null;

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

export default function App() {
  const [data, setData] = useState<Snapshot>(preview);
  const [view, setView] = useState<View>('environment');
  const [localBusy, setBusy] = useState<string | null>(null);
  const [backendBusy, setBackendBusy] = useState<string | null>(null);
  const busy = backendBusy ?? localBusy;
  const [notice, setNotice] = useState<Notice>(null);
  const [log, setLog] = useState<{ service: ServiceId; text: string } | null>(null);
  const [help, setHelp] = useState(false);
  const [ports, setPorts] = useState<Ports>(preview.ports);
  const [phpVersion, setPhpVersion] = useState(preview.phpVersion);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const operation = useRef(false);
  const backendOperation = useRef(false);
  const versions = data.runtimeVersions;
  const selectedPhp = data.phpVersions[data.phpVersion] ?? versions?.php;
  const allReady = data.services.every(service => service.ready);
  const anyRunning = data.services.some(service => service.running);
  const usable = isDesktop && data.runtimeReady && !busy;

  const refresh = useCallback(async () => {
    if (!isDesktop || operation.current) return;
    const current = await invoke<Snapshot>('snapshot');
    setData(current);
  }, []);

  useEffect(() => {
    let disposed = false;
    let fetching = false;
    const poll = async () => {
      if (fetching || operation.current || disposed || !isDesktop) return;
      fetching = true;
      try {
        const current = await invoke<Snapshot>('snapshot');
        if (!disposed) setData(current);
      } catch (error) {
        if (!disposed) setNotice({ kind: 'error', text: message(error) });
      } finally { fetching = false; }
    };
    void poll();
    const unlisteners: (() => void)[] = [];
    void Promise.all([
      listen('fairway-status', payload => setData(payload as Snapshot)),
      listen('fairway-operation-started', payload => {
        backendOperation.current = true; setBackendBusy(String(payload));
      }),
      listen('fairway-operation-finished', () => {
        backendOperation.current = false; setBackendBusy(null);
      }),
      listen('fairway-error', payload => {
        setBusy(null); operation.current = false;
        setNotice({ kind: 'error', text: String(payload) });
      }),
      listen('fairway-shutdown', () => {
        backendOperation.current = true; setBackendBusy('Stopping services before quitting…');
      }),
    ]).then(items => {
      if (disposed) items.forEach(fn => fn()); else unlisteners.push(...items);
    }).catch(error => { if (!disposed) setNotice({ kind: 'error', text: message(error) }); });
    return () => { disposed = true; unlisteners.forEach(fn => fn()); };
  }, []);

  useEffect(() => { setPorts(data.ports); }, [data.ports.website, data.ports.phpmyadmin, data.ports.mysql, data.ports.php]);
  useEffect(() => { setPhpVersion(data.phpVersion); }, [data.phpVersion]);

  async function run(label: string, action: () => Promise<void>) {
    if (operation.current || backendOperation.current) return;
    operation.current = true;
    setBusy(label); setNotice(null);
    try { await action(); }
    catch (error) { setNotice({ kind: 'error', text: message(error) }); }
    finally {
      operation.current = false; setBusy(null);
      try { await refresh(); } catch { /* The first operation error remains visible. */ }
    }
  }

  function serviceAction(action: string, service?: ServiceId) {
    void run(action.startsWith('stop') ? 'Stopping services…' : action === 'restart' ? 'Restarting service…' : 'Starting services…', async () => {
      setData(await invoke<Snapshot>('service_action', { action, service: service ?? null }));
    });
  }

  function open(target: string) {
    void run('Opening…', () => invoke<void>('open_target', { target }));
  }

  async function copy(text: string, key: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(key); window.setTimeout(() => setCopied(null), 1800);
    } catch (error) { setNotice({ kind: 'error', text: `Could not copy: ${message(error)}` }); }
  }

  function showLog(service: ServiceId) {
    void run('Reading log…', async () => setLog({ service, text: await invoke<string>('read_log', { service }) }));
  }

  const connection = `DB_CONNECTION=mysql\nDB_HOST=127.0.0.1\nDB_PORT=${data.ports.mysql}\nDB_DATABASE=fairway\nDB_USERNAME=fairway\nDB_PASSWORD=fairway`;

  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="#" onClick={event => { event.preventDefault(); setView('environment'); }} aria-label="Fairway XMP home">
        <span className="brand-mark"><Leaf size={23} strokeWidth={2.2} /></span><span className="brand-name">fairway<span className="brand-period">.</span><span className="brand-edition">XMP</span></span>
      </a>
      <div className="sidebar-label">DEVELOPMENT ENVIRONMENT</div>
      <nav aria-label="Main navigation">
        <button className={view === 'environment' ? 'nav-item active' : 'nav-item'} onClick={() => setView('environment')}><Globe size={18} />Environment</button>
        <button className={view === 'settings' ? 'nav-item active' : 'nav-item'} onClick={() => setView('settings')}><Settings2 size={18} />Settings</button>
        <button className={view === 'updates' ? 'nav-item active' : 'nav-item'} onClick={() => setView('updates')}><ArrowDownToLine size={18} />Updates</button>
      </nav>
      <div className="sidebar-bottom">
        <div className="local-badge"><ShieldCheck size={16} /><div>Made for local learning<span>Everything stays on your computer</span></div></div>
        <button className="help-button" onClick={() => setHelp(true)}><CircleHelp size={17} />Getting started<ArrowUpRight size={14} /></button>
        <div className="sidebar-footer">FAIRWAY XMP <span>v{data.version}</span></div>
      </div>
    </aside>

    <main>
      <header className="topbar"><span>Workspace <ChevronRight size={13} />{view === 'environment' ? 'Environment' : view === 'settings' ? 'Settings' : 'Updates'}</span><span className="local-pill"><span />Local environment</span></header>
      <div className="main-content">
        {!isDesktop && <div className="preview-banner"><Code2 size={16} />Interface preview — open the desktop app to manage services.</div>}
        {notice && <div className={`notice ${notice.kind}`} role={notice.kind === 'error' ? 'alert' : 'status'}><span>{notice.text}</span><button aria-label="Dismiss message" onClick={() => setNotice(null)}><X size={17} /></button></div>}

        {view === 'environment' && <>
          <div className="page-heading"><div><div className="eyebrow">YOUR LOCAL WORKSPACE</div><h1>Ready when you are.</h1><p>Everything you need to build with PHP, in one place.</p></div><button className="primary" disabled={anyRunning ? !isDesktop || !!busy : !usable} onClick={() => serviceAction(anyRunning ? 'stop_all' : 'start_all')}>{busy ? <LoaderCircle className="spin" size={16} /> : anyRunning ? <Square size={14} /> : <Play size={15} fill="currentColor" />}{anyRunning ? 'Stop all' : 'Start all'}</button></div>
          <div className="section-heading"><h2>Services <span>4</span></h2><span className="stack-status"><span className={`status-dot ${allReady ? 'running' : ''}`} />{allReady ? 'All services ready' : anyRunning ? 'Environment partially running' : 'All services stopped'}</span></div>
          <div className="services-grid">{(['nginx', 'php', 'mysql', 'phpmyadmin'] as ServiceId[]).map(id => {
            const service = data.services.find(s => s.id === id)!;
            return <ServiceCard key={id} service={service} version={id === 'php' ? selectedPhp : versions?.[meta[id].version]} disabled={!usable} busy={!!busy} onAction={serviceAction} onOpen={open} onLog={showLog} />;
          })}</div>
          {isDesktop && !data.runtimeReady && <div className="notice error" role="alert">Fairway XMP’s PHP or database files are missing. Reinstall Fairway XMP to restore them. Your projects and database data will be kept.</div>}

          <div className="workspace-card">
            <div className="workspace-icon"><FolderOpen size={23} /></div>
            <div className="workspace-details"><h2>A home for your projects</h2><p>Put your PHP projects in your document root.</p><code title={data.documentRoot}>{data.documentRoot}</code></div>
            <button className="secondary" disabled={!isDesktop || !!busy} onClick={() => open('coursework')}>Open folder<ArrowUpRight size={15} /></button>
          </div>

          <div className="bottom-grid">
            <section className="connection-card"><div className="card-title"><h2><Database size={17} />Database connection</h2><button className="icon-button" title="Copy database environment settings" aria-label="Copy database environment settings" onClick={() => { void copy(connection, 'database'); }}>{copied === 'database' ? <Check size={16} /> : <Clipboard size={16} />}</button></div><dl><div><dt>Host</dt><dd>127.0.0.1<span className="port-inline">:{data.ports.mysql}</span></dd></div><div><dt>Database</dt><dd>fairway</dd></div><div><dt>Username / password</dt><dd>fairway / fairway</dd></div></dl><button className="text-link" disabled={!data.services.find(s => s.id === 'mysql')?.ready || !!busy} onClick={() => { void run('Backing up databases…', async () => { const path = await invoke<string>('backup_database'); setNotice({ kind: 'success', text: `Database backup saved: ${path}` }); }); }}><Download size={14} />Back up databases</button></section>
            <section className="terminal-card"><span className="terminal-symbol"><Terminal size={20} /></span><h2>Keep your usual workflow.</h2><p>PHP and Composer, ready in your terminal.<br />Your code. Your editor. Your pace.</p><button className="secondary" disabled={!isDesktop || !data.pathInstalled || !!busy} onClick={() => open('terminal')}>Open terminal<ArrowUpRight size={15} /></button></section>
          </div>
          <footer className="content-footer"><span><span className="status-dot running" />127.0.0.1 · Local access only</span><button onClick={() => setView('settings')}>Configure environment<Settings2 size={13} /></button></footer>
        </>}

        {view === 'settings' && <>
          <div className="page-heading"><div><div className="eyebrow">MAKE IT YOURS</div><h1>Environment settings.</h1><p>Predictable defaults, with room to adjust.</p></div></div>
          <section className="settings-card"><h2>Service Settings</h2><p>Stop all services before changing ports or PHP. Saved settings stay the same across restarts.</p>
            <form onSubmit={event => { event.preventDefault(); void run('Saving settings…', async () => {
              const mysqlPortChanged = ports.mysql !== data.ports.mysql;
              setData(await invoke<Snapshot>('save_settings', { ports, version: phpVersion })); setNotice({ kind: 'success', text: mysqlPortChanged ? 'Settings saved. Update existing project connection settings for the new MySQL port.' : 'Settings saved.' });
            }); }}>
              <div className="port-fields">{([['website', 'Website', 'Nginx'], ['phpmyadmin', 'phpMyAdmin', 'Database browser'], ['mysql', 'MySQL', 'Database connection']] as const).map(([key, label, hint]) => <label key={key}><span>{label}<small>{hint}</small></span><input type="number" min="1024" max="65535" required value={ports[key]} disabled={anyRunning || !!busy} onChange={event => setPorts({ ...ports, [key]: Number(event.target.value) })} /></label>)}</div>
              <details className="advanced"><summary>Advanced settings</summary>
                <label><span>PHP version<small>For websites, CLI PHP, and Composer. PHP 8.5 is the default.</small></span><select value={phpVersion} disabled={!isDesktop || !!busy || anyRunning || !data.runtimeReady} onChange={event => setPhpVersion(event.target.value)}>{Object.keys(data.phpVersions).sort().map(version => <option key={version} value={version}>PHP {version}</option>)}</select></label>
                <label><span>PHP FastCGI port<small>Internal connection between Nginx and PHP</small></span><input type="number" min="1024" max="65535" required value={ports.php} disabled={anyRunning || !!busy} onChange={event => setPorts({ ...ports, php: Number(event.target.value) })} /></label>
              </details>
              {anyRunning && <p className="settings-hint">Services are running. Stop all from Environment to edit settings.</p>}
              <button className="primary" disabled={!isDesktop || anyRunning || !!busy || !data.runtimeReady} type="submit">Save</button>
            </form>
          </section>
          <section className="settings-card"><div className="card-title"><h2>Command-line tools</h2><span className={`status-label ${data.pathInstalled ? 'ready' : ''}`}>{data.pathInstalled ? 'Enabled' : 'Not enabled'}</span></div><p>Add PHP and Composer to your user PATH. Restart existing terminals and VS Code after changes.</p><dl className="path-list"><div><dt>Current PHP</dt><dd>{data.pathPhp ?? 'Not found'}</dd></div><div><dt>Current Composer</dt><dd>{data.pathComposer ?? 'Not found'}</dd></div></dl><p className="settings-hint">If an older PHP installation appears here, close and reopen Fairway XMP and your terminal. “Open terminal” uses Fairway XMP’s PHP and Composer.</p><button className="secondary" disabled={!isDesktop || !!busy || (!data.pathInstalled && !data.runtimeReady)} onClick={() => { void run('Updating user PATH…', async () => {
            setData(await invoke<Snapshot>('cli_integration', { enabled: !data.pathInstalled })); setNotice({ kind: 'success', text: 'Command-line integration updated. Reopen your terminals and VS Code.' });
          }); }}>{data.pathInstalled ? 'Remove from user PATH' : 'Enable PHP and Composer'}</button></section>
          <section className="settings-card"><h2>Your files</h2><dl className="path-list"><div><dt>Document root</dt><dd>{data.documentRoot}</dd></div><div><dt>Settings and database data</dt><dd>{data.dataDir}</dd></div></dl><div className="button-row"><button className="secondary" disabled={!isDesktop || !!busy} onClick={() => open('logs')}><ScrollText size={15} />Open logs</button><button className="secondary" disabled={!isDesktop || !!busy} onClick={() => open('backups')}><FolderOpen size={15} />Open backups</button></div></section>
        </>}

        {view === 'updates' && <>
          <div className="page-heading"><div><div className="eyebrow">KEEP YOUR TOOLS CURRENT</div><h1>A dependable environment.</h1><p>Find new versions of Fairway XMP and your bundled tools.</p></div></div>
          <section className="settings-card"><div className="card-title"><h2>Fairway XMP {data.version}</h2><span className="status-label">Installed version</span></div><div className="runtime-list">{['php', 'nginx', 'mysql', 'phpmyadmin', 'composer'].map(name => <div key={name}><span>{name === 'php' ? 'PHP' : name === 'mysql' ? 'MySQL' : name === 'phpmyadmin' ? 'phpMyAdmin' : name === 'nginx' ? 'Nginx' : 'Composer'}</span><code>{name === 'php' ? selectedPhp ?? 'Unavailable' : versions?.[name] ?? 'Unavailable'}</code></div>)}</div><p>{update?.configured ? 'Updates preserve your projects and database data. Fairway XMP backs up student databases and stops services before installing an update.' : 'Download a newer installer from View downloads. Back up your databases, stop all services, and quit Fairway XMP before installing. Your projects and database data are kept.'}</p><div className="button-row update-actions"><button className="primary" disabled={!isDesktop || !!busy} onClick={() => { void run('Checking for updates…', async () => {
            const result = await invoke<UpdateInfo>('check_update'); setUpdate(result);
            if (result.configured && !result.version) setNotice({ kind: 'success', text: 'You’re using the latest available release.' });
            if (!result.configured) setNotice({ kind: 'success', text: 'This version uses manual updates. Choose View downloads to find a newer installer.' });
          }); }}><RefreshCw size={15} />Check for updates</button>
            <button className="secondary" disabled={!isDesktop || !!busy} onClick={() => open('releases')}>View downloads<ExternalLink size={15} /></button><button className="secondary" disabled={!isDesktop || !!busy} onClick={() => open('source')}>Source code<ExternalLink size={15} /></button><button className="secondary" disabled={!isDesktop || !!busy} onClick={() => open('issues')}>Get help<ExternalLink size={15} /></button></div>
            {update && !update.configured && <p className="settings-hint">Visit the downloads page to see available releases.</p>}
            {update?.version && <div className="update-available"><h3>Version {update.version} is available</h3><p className="release-notes">{update.notes ?? 'A tested environment update is ready.'}</p><button className="primary" disabled={!!busy} onClick={() => { void run('Backing up and installing update…', () => invoke<void>('install_update')); }}>Install update</button></div>}
          </section>
        </>}
      </div>
      {busy && <div className="operation-status" role="status"><LoaderCircle className="spin" size={16} />{busy}</div>}
    </main>

    {log && <Modal title={`${meta[log.service].name} logs`} onClose={() => setLog(null)}><pre className="log-output">{log.text}</pre><button className="secondary" disabled={!!busy} onClick={() => showLog(log.service)}><RefreshCw size={15} />Refresh log</button></Modal>}
    {help && <Modal title="Your first PHP project" onClose={() => setHelp(false)}><ol className="help-steps"><li>Click <strong>Start all</strong> and wait for the services to become ready.</li><li>Open your coursework folder: <code>fairway/www</code>.</li><li>Create a folder called <code>lesson-01</code> and add an <code>index.php</code> file.</li><li>Open <code>http://127.0.0.1:{data.ports.website}/lesson-01/</code> in your browser.</li></ol><p>Log into phpMyAdmin with username <code>fairway</code> and password <code>fairway</code>. Reopen your terminal after installation to use PHP and Composer. If they are unavailable, enable them in Settings.</p><p className="settings-hint">Closing this window keeps services running. Use the Fairway XMP menu bar or system tray icon to reopen the panel, control services, or quit. Quitting stops all services.</p></Modal>}
  </div>;
}

function ServiceCard({ service, version, disabled, busy, onAction, onOpen, onLog }: {
  service: ServiceStatus; version?: string; disabled: boolean; busy: boolean;
  onAction: (action: string, service?: ServiceId) => void;
  onOpen: (target: string) => void; onLog: (service: ServiceId) => void;
}) {
  const item = meta[service.id];
  const Icon = item.icon;
  const browser = service.id === 'phpmyadmin';
  return <section className="service-card">
    <div className="service-top"><span className={`service-icon ${item.color}`}><Icon size={23} strokeWidth={1.8} /></span><span className={`status-label ${service.ready ? 'ready' : service.running ? 'pending' : ''}`}><span className={`status-dot ${service.ready ? 'running' : service.running ? 'pending' : ''}`} />{service.ready ? 'Running' : service.running ? 'Waiting' : 'Stopped'}</span></div>
    <div className="service-name"><h3>{item.name}</h3>{version && <span>{version}</span>}</div><p>{item.description}</p>
    <div className="service-address"><span>{service.id === 'php' ? 'Internal port' : browser || service.id === 'nginx' ? 'localhost' : '127.0.0.1'}<span className="address-port">:{service.port}</span></span>{(browser || service.id === 'nginx') && <button className="icon-button" title={`Open ${item.name}`} aria-label={`Open ${item.name}`} disabled={!service.ready || busy} onClick={() => onOpen(browser ? 'phpmyadmin' : 'website')}><ExternalLink size={14} /></button>}</div>
    {service.message && <div className="service-error" role="status">{service.message}</div>}
    <div className="service-actions">{browser ? <button className="service-start" disabled={!service.ready || busy} onClick={() => onOpen('phpmyadmin')}>Open phpMyAdmin<ArrowUpRight size={14} /></button> : service.id === 'php' ? <span className="service-managed">Managed with Nginx</span> : <><button className="service-start" disabled={disabled} onClick={() => onAction(service.running ? 'stop' : 'start', service.id)}>{service.running ? <Square size={12} /> : <Play size={12} />}{service.running ? 'Stop service' : 'Start service'}</button><button className="icon-button" title={`Restart ${item.name}`} aria-label={`Restart ${item.name}`} disabled={disabled || !service.running} onClick={() => onAction('restart', service.id)}><RefreshCw size={14} /></button></>}<button className="icon-button" title={`View ${item.name} logs`} aria-label={`View ${item.name} logs`} disabled={!isDesktop || busy} onClick={() => onLog(service.id)}><ScrollText size={15} /></button></div>
  </section>;
}

function Modal({ title, onClose, children }: { title: string; onClose: () => void; children: React.ReactNode }) {
  const container = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  useEffect(() => { close.current = onClose; }, [onClose]);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    container.current?.querySelector<HTMLButtonElement>('button')?.focus();
    function key(event: KeyboardEvent) {
      if (event.key === 'Escape') close.current();
      if (event.key === 'Tab') {
        const focusable = container.current?.querySelectorAll<HTMLElement>('button:not(:disabled), a, input');
        if (!focusable?.length) return;
        const first = focusable[0], last = focusable[focusable.length - 1];
        if (event.shiftKey && document.activeElement === first) { event.preventDefault(); last.focus(); }
        else if (!event.shiftKey && document.activeElement === last) { event.preventDefault(); first.focus(); }
      }
    }
    document.addEventListener('keydown', key);
    return () => { document.removeEventListener('keydown', key); previous?.focus(); };
  }, []);
  return <div className="modal-backdrop" onClick={event => { if (event.target === event.currentTarget) onClose(); }}><div ref={container} className="modal" role="dialog" aria-modal="true" aria-label={title}><div className="card-title"><h2>{title}</h2><button className="icon-button" aria-label="Close dialog" onClick={onClose}><X size={20} /></button></div>{children}</div></div>;
}
