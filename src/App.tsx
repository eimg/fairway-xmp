import { Activity, lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import {
  ArrowDownToLine, ArrowUpRight, Check, ChevronDown, ChevronRight, CircleHelp, Clipboard, Code2,
  Database, Download, ExternalLink, FolderOpen, Globe, Leaf, LoaderCircle, Lock, Play,
  Plus, RefreshCw, ScrollText, Settings2, Square, Terminal, X,
} from 'lucide-react';
import { canControl, invoke, isDesktop, isUiTest, listen, preview } from './bridge';
import type { Ports, Project, ProjectConfig, ServiceId, ServiceStatus, Snapshot, UpdateInfo } from './types';
import HelpTip from './HelpTip';
import DataBrowser from './data/DataBrowser';

const UiTestToolbar = isUiTest ? lazy(() => import('./ui-test/Toolbar')) : null;
const TerminalPage = lazy(() => import('./TerminalPage'));
const dataBrowserAvailable = isUiTest || isDesktop;

const meta = {
  nginx: { name: 'Nginx', description: 'Your local web server', icon: Globe, version: 'nginx', color: 'green' },
  php: { name: 'PHP', description: 'Managed automatically with Nginx', icon: Code2, version: 'php', color: 'purple' },
  mysql: { name: 'MySQL', description: 'Your local database', icon: Database, version: 'mysql', color: 'blue' },
  phpmyadmin: { name: 'phpMyAdmin', description: 'Manage tables and data', icon: Database, version: 'phpmyadmin', color: 'orange' },
} as const;

type View = 'environment' | 'settings' | 'updates' | 'data' | 'terminal';
type Notice = { text: string; kind: 'error' | 'success' } | null;

function message(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

type ProjectForm = ProjectConfig & { previous: string | null; domainEdited: boolean; error: string | null };

// Suggest a DNS-safe .test label from a display name.
function suggestDomain(name: string): string {
  return name.toLowerCase().replace(/[^a-z0-9]+/g, '-').replace(/^-+|-+$/g, '').slice(0, 63).replace(/-+$/, '');
}

const hostsApproval: Record<string, string> = {
  macos: 'macOS will ask for an administrator password.',
  windows: 'Windows will ask for administrator permission.',
};
const trustApproval: Record<string, string> = {
  macos: 'Fairway adds its local certificate to your login keychain. macOS asks for your password to approve the trust setting; an administrator account is not required.',
  windows: 'Fairway adds its local certificate to your Windows user certificate store. Windows shows a security warning; choose Yes to continue. An administrator account is not required.',
};

export default function App() {
  const [data, setData] = useState<Snapshot>(preview);
  const [view, setView] = useState<View>('environment');
  const [terminalOpened, setTerminalOpened] = useState(false);
  const [terminalFocus, setTerminalFocus] = useState(0);
  const [localBusy, setBusy] = useState<string | null>(null);
  const [backendBusy, setBackendBusy] = useState<string | null>(null);
  const busy = backendBusy ?? localBusy;
  const databaseBusy = backendBusy === 'Working with database…';
  const [notice, setNotice] = useState<Notice>(null);
  const [report, setReport] = useState<string | null>(null);
  const [log, setLog] = useState<{ service: ServiceId; text: string } | null>(null);
  const [help, setHelp] = useState(false);
  const [ports, setPorts] = useState<Ports>(preview.ports);
  const [phpVersion, setPhpVersion] = useState(preview.phpVersion);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [projectsExpanded, setProjectsExpanded] = useState(true);
  const [projectForm, setProjectForm] = useState<ProjectForm | null>(null);
  const [removing, setRemoving] = useState<Project | null>(null);
  const [sslSetup, setSslSetup] = useState<Project | null>(null);
  const operation = useRef(false);
  const backendOperation = useRef(false);
  const noticeRef = useRef<HTMLDivElement>(null);
  const versions = data.runtimeVersions;
  const selectedPhp = data.phpVersions[data.phpVersion] ?? versions?.php;
  const allReady = data.services.every(service => service.ready);
  const anyRunning = data.services.some(service => service.running);
  const usable = canControl && data.runtimeReady && !busy;
  const databaseCliAvailable = isDesktop || isUiTest;
  const cliTools = databaseCliAvailable ? 'PHP, Composer, and database commands' : 'PHP and Composer commands';

  useEffect(() => {
    const key = (event: KeyboardEvent) => {
      if (!event.ctrlKey || event.altKey || event.metaKey || event.code !== 'Backquote') return;
      if (document.querySelector('[aria-modal="true"]')) return;
      event.preventDefault(); event.stopPropagation();
      if (busy && !terminalOpened) return;
      setTerminalOpened(true); setView('terminal'); setTerminalFocus(value => value + 1);
    };
    document.addEventListener('keydown', key, true);
    return () => document.removeEventListener('keydown', key, true);
  }, [busy, terminalOpened]);

  const refresh = useCallback(async () => {
    if (!canControl || operation.current) return;
    const current = await invoke<Snapshot>('snapshot');
    setData(current);
  }, []);

  useEffect(() => {
    let disposed = false;
    let fetching = false;
    const poll = async () => {
      if (fetching || operation.current || disposed || !canControl) return;
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
      ...(isUiTest ? [listen('ui-test-reset', payload => {
        const current = payload as Snapshot;
        setPorts(current.ports); setPhpVersion(current.phpVersion);
        setNotice(null); setUpdate(null); setLog(null); setReport(null); setHelp(false); setCopied(null);
      })] : []),
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

  // Results appear at the top of the page; bring them into view from lower cards.
  useEffect(() => { noticeRef.current?.scrollIntoView?.({ block: 'nearest', behavior: 'smooth' }); }, [notice]);
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
    if (target === 'terminal') { setTerminalOpened(true); setView('terminal'); setTerminalFocus(value => value + 1); return; }
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

  function projectUrl(project: Project) {
    return project.https ? `https://${project.domain}.test:8443/` : `http://${project.domain}.test:${data.ports.website}/`;
  }

  function setHttps(project: Project, enabled: boolean) {
    void run('Updating project HTTPS…', async () => {
      setData(await invoke<Snapshot>('set_project_https', { domain: project.domain, enabled }));
      setNotice({ kind: 'success', text: enabled ? `HTTPS is ready at https://${project.domain}.test:8443/.` : `HTTPS is off for ${project.domain}.test. Its HTTP address still works.` });
    });
  }

  function saveProject(form: ProjectForm) {
    const { previous, name, domain, root, https, indexFallback } = form;
    const project: ProjectConfig = { name, domain, root, https, indexFallback };
    void run('Saving project…', async () => {
      try {
        setData(await invoke<Snapshot>('save_project', { previous, project }));
        setProjectForm(null);
        setNotice({ kind: 'success', text: `${project.name} is ready at http://${project.domain}.test:${data.ports.website}/.` });
      } catch (error) {
        // A saved project with a pending hosts update is shown in the list with a retry.
        if (message(error).startsWith('The project was saved')) { setProjectForm(null); throw error; }
        setProjectForm({ ...form, error: message(error) });
      }
    });
  }

  async function chooseRoot(form: ProjectForm) {
    try {
      const root = await invoke<string | null>('pick_folder');
      if (!root) return;
      const folder = root.split(/[\\/]/).filter(Boolean).pop() ?? '';
      const parent = root.split(/[\\/]/).filter(Boolean).slice(-2, -1)[0] ?? '';
      const name = form.name || (folder === 'public' && parent ? parent : folder);
      setProjectForm({ ...form, root, name, domain: form.domainEdited ? form.domain : suggestDomain(name) });
    } catch (error) { setProjectForm({ ...form, error: message(error) }); }
  }

  const connection = `DB_CONNECTION=mysql\nDB_HOST=127.0.0.1\nDB_PORT=${data.ports.mysql}\nDB_DATABASE=fairway\nDB_USERNAME=fairway\nDB_PASSWORD=fairway`;

  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="#" onClick={event => { event.preventDefault(); setView('environment'); }} aria-label="Fairway XMP home">
        <span className="brand-mark"><Leaf size={23} strokeWidth={2.2} /></span><span className="brand-name">fairway<span className="brand-period">.</span><span className="brand-edition">XMP</span></span>
      </a>
      <nav aria-label="Main navigation">
        <button className={view === 'environment' ? 'nav-item active' : 'nav-item'} onClick={() => setView('environment')}><Globe size={18} />Environment</button>
        {dataBrowserAvailable && <button className={view === 'data' ? 'nav-item active' : 'nav-item'} disabled={!!busy && !databaseBusy} onClick={() => setView('data')}><Database size={18} />Data browser</button>}
        <button className={view === 'terminal' ? 'nav-item active' : 'nav-item'} title="Terminal (Ctrl+`)" disabled={!!busy && !terminalOpened} onClick={() => open('terminal')}><Terminal size={18} />Terminal</button>
        <button className={view === 'settings' ? 'nav-item active' : 'nav-item'} onClick={() => setView('settings')}><Settings2 size={18} />Settings</button>
        <button className={view === 'updates' ? 'nav-item active' : 'nav-item'} onClick={() => setView('updates')}><ArrowDownToLine size={18} />Updates</button>
      </nav>
      <div className="sidebar-bottom">
        <button className="help-button" onClick={() => setHelp(true)}><CircleHelp size={17} />Getting started<ArrowUpRight size={14} /></button>
        <div className="sidebar-footer" aria-label="App version">v{data.version}</div>
      </div>
    </aside>

    <main>
      <header className="topbar"><span>Workspace <ChevronRight size={13} />{view === 'environment' ? 'Environment' : view === 'settings' ? 'Settings' : view === 'data' ? 'Data browser' : view === 'terminal' ? 'Terminal' : 'Updates'}</span><span className="local-pill"><span />Local environment</span></header>
      <div className="main-content">
        {UiTestToolbar && <Suspense fallback={null}><UiTestToolbar busy={!!busy} /></Suspense>}
        {!isDesktop && !isUiTest && <div className="preview-banner"><Code2 size={16} />Interface preview — open the desktop app to manage services.</div>}
        {notice && <div ref={noticeRef} className={`notice ${notice.kind}`} role={notice.kind === 'error' ? 'alert' : 'status'}><span>{notice.text}</span><button aria-label="Dismiss message" onClick={() => setNotice(null)}><X size={17} /></button></div>}
        {terminalOpened && <div hidden={view !== 'terminal'}><Suspense fallback={<p>Loading terminal…</p>}><TerminalPage active={view === 'terminal'} busy={!!busy} focusRequest={terminalFocus} /></Suspense></div>}
        {dataBrowserAvailable && <Activity mode={view === 'data' ? 'visible' : 'hidden'}><DataBrowser ready={data.services.some(service => service.id === 'mysql' && service.ready)} busy={!!busy && !databaseBusy} start={() => serviceAction('start', 'mysql')} open={open} /></Activity>}

        {view === 'environment' && <>
          <div className="page-heading"><div><h1>Ready when you are.</h1><p>Everything you need to build with PHP, in one place.</p></div><button className="primary" disabled={anyRunning ? !canControl || !!busy : !usable} onClick={() => serviceAction(anyRunning ? 'stop_all' : 'start_all')}>{busy ? <LoaderCircle className="spin" size={16} /> : anyRunning ? <Square size={14} /> : <Play size={15} fill="currentColor" />}{anyRunning ? 'Stop all' : 'Start all'}</button></div>
          <div className="section-heading"><h2>Services <span>4</span></h2><span className="stack-status"><span className={`status-dot ${allReady ? 'running' : ''}`} />{allReady ? 'All services ready' : anyRunning ? 'Environment partially running' : 'All services stopped'}</span></div>
          <div className="services-grid">{(['nginx', 'php', 'mysql', 'phpmyadmin'] as ServiceId[]).map(id => {
            const service = data.services.find(s => s.id === id)!;
            return <ServiceCard key={id} service={service} version={id === 'php' ? selectedPhp : versions?.[meta[id].version]} disabled={!usable} busy={!!busy} onAction={serviceAction} onOpen={open} onLog={showLog} />;
          })}</div>
          {canControl && !data.runtimeReady && <div className="notice error" role="alert">Fairway XMP’s PHP or database files are missing. Reinstall Fairway XMP to restore them. Your projects and database data will be kept.</div>}

          <div className="workspace-card">
            <div className="workspace-icon"><FolderOpen size={23} /></div>
            <div className="workspace-details"><h2>A home for your projects</h2><p>Put your PHP projects in your document root.</p><code title={data.documentRoot}>{data.documentRoot}</code></div>
            <button className="secondary" disabled={!canControl || !!busy} onClick={() => open('coursework')}>Open folder<ArrowUpRight size={15} /></button>
          </div>

          <section className="project-card">
            <div className="card-title"><h2><Globe size={17} />Projects</h2><div className="project-heading-actions"><button className="secondary" disabled={!usable} onClick={() => setProjectForm({ previous: null, name: '', domain: '', root: '', https: false, indexFallback: false, domainEdited: false, error: null })}><Plus size={14} />Add project</button><button className="secondary project-collapse" aria-label={projectsExpanded ? 'Collapse projects' : 'Expand projects'} aria-controls="project-sites-content" aria-expanded={projectsExpanded} onClick={() => setProjectsExpanded(expanded => !expanded)}><ChevronDown size={16} className={projectsExpanded ? 'expanded' : ''} /></button></div></div>
            <div id="project-sites-content" hidden={!projectsExpanded}>
              <p>Add a project to serve any folder at its own <code>.test</code> address, with optional HTTPS. Everything in your document root still opens at <code>http://127.0.0.1:{data.ports.website}/</code>.</p>
              {data.domainsPending && <div className="project-pending" role="status"><span>Some project addresses aren’t set up on this computer yet.</span><button className="secondary" disabled={!usable} onClick={() => { void run('Updating the hosts file…', async () => { setData(await invoke<Snapshot>('sync_domains')); setNotice({ kind: 'success', text: 'Project addresses are set up.' }); }); }}>Set up addresses</button></div>}
              {data.projects.length === 0 && <p className="settings-hint">No projects yet. Choose Add project to serve a folder anywhere on this computer.</p>}
              {data.projects.length > 0 && <div className="project-table-scroll"><table className="project-table" aria-label="Projects">
                <thead><tr><th scope="col">Project</th><th scope="col">Address</th><th scope="col">HTTPS</th><th scope="col" aria-label="Actions" /></tr></thead>
                <tbody>{data.projects.map(project => <tr key={project.domain}>
                  <th scope="row"><strong title={project.root}>{project.name}</strong>{!project.available && <small className="project-warning">Unavailable: the web root folder is missing. Edit to fix the path.</small>}</th>
                  <td><span className="project-address">{project.https && <Lock size={11} aria-label="HTTPS" className="project-lock" />}<code title={projectUrl(project)}>{project.domain}.test:{project.https ? 8443 : data.ports.website}</code><button className="icon-button" aria-label={`Open ${project.name}`} title={`Open ${projectUrl(project)}`} disabled={!project.available || !project.domainReady || !data.services.find(service => service.id === 'nginx')?.ready || !!busy} onClick={() => open(`project:${project.domain}`)}><ExternalLink size={13} /></button></span>{!project.domainReady && <small className="project-warning">Address not set up yet</small>}</td>
                  <td><label className="ssl-toggle"><input type="checkbox" role="switch" aria-label={`HTTPS for ${project.name}`} checked={project.https} disabled={!usable || (!project.https && (!project.available || !project.domainReady))} onChange={event => { if (!event.target.checked) setHttps(project, false); else if (data.httpsTrusted) setHttps(project, true); else setSslSetup(project); }} /><span className="ssl-track" aria-hidden="true" /></label></td>
                  <td><div className="project-actions"><button className="secondary" aria-label={`Edit ${project.name}`} disabled={!usable} onClick={() => setProjectForm({ ...project, previous: project.domain, domainEdited: true, error: null })}>Edit</button><button className="secondary" aria-label={`Remove ${project.name}`} disabled={!usable} onClick={() => setRemoving(project)}>Remove</button></div></td>
                </tr>)}</tbody>
              </table></div>}
              {data.httpsTrusted && !data.projects.some(project => project.https) && <div className="project-pending"><span>Fairway's local HTTPS certificate is still trusted by this computer. Remove the trust if you no longer need HTTPS.</span><button className="secondary" disabled={!usable} onClick={() => { void run('Removing certificate trust…', async () => { setData(await invoke<Snapshot>('remove_https_trust')); setNotice({ kind: 'success', text: 'Fairway’s local certificate is no longer trusted.' }); }); }}>Remove certificate trust</button></div>}
            </div>
          </section>

          <div className="bottom-grid">
            <section className="connection-card"><div className="card-title"><h2><Database size={17} />Database connection</h2><button className="icon-button" title="Copy database environment settings" aria-label="Copy database environment settings" onClick={() => { void copy(connection, 'database'); }}>{copied === 'database' ? <Check size={16} /> : <Clipboard size={16} />}</button></div><dl><div><dt>Host</dt><dd>127.0.0.1<span className="port-inline">:{data.ports.mysql}</span></dd></div><div><dt>Database</dt><dd>fairway</dd></div><div><dt>Username / password</dt><dd>fairway / fairway</dd></div></dl><button className="text-link" disabled={!data.services.find(s => s.id === 'mysql')?.ready || !!busy} onClick={() => { void run('Backing up databases…', async () => { const path = await invoke<string>('backup_database'); setNotice({ kind: 'success', text: `Database backup saved: ${path}` }); }); }}><Download size={14} />Back up databases</button></section>
            <section className="terminal-card"><span className="terminal-symbol"><Terminal size={20} /></span><h2>Your tools, right here.</h2><p>Run PHP and Composer in the embedded terminal.<br />Ready in your coursework folder.</p><button className="secondary" disabled={!canControl || !data.runtimeReady || !!busy} onClick={() => open('terminal')}>Open terminal<ChevronRight size={15} /></button></section>
          </div>
          <footer className="content-footer"><span><span className="status-dot running" />127.0.0.1 · Local access only</span><button onClick={() => setView('settings')}>Configure environment<Settings2 size={13} /></button></footer>
        </>}

        {view === 'settings' && <>
          <div className="page-heading"><div><h1>Environment settings.</h1><p>Predictable defaults, with room to adjust.</p></div></div>
          <section className="settings-card"><h2>Service Settings</h2><p>Stop all services before changing ports or PHP. Saved settings stay the same across restarts.</p>
            <form onSubmit={event => { event.preventDefault(); void run('Saving settings…', async () => {
              const mysqlPortChanged = ports.mysql !== data.ports.mysql;
              setData(await invoke<Snapshot>('save_settings', { ports, version: phpVersion })); setNotice({ kind: 'success', text: mysqlPortChanged ? 'Settings saved. Update existing project connection settings for the new MySQL port.' : 'Settings saved.' });
            }); }}>
              <div className="port-fields">{([['website', 'Website', 'Nginx'], ['phpmyadmin', 'phpMyAdmin', 'Database browser'], ['mysql', 'MySQL', 'Database connection']] as const).map(([key, label, hint]) => <label key={key}><span>{label}<small>{hint}</small></span><input type="number" min="1024" max="65535" required value={ports[key]} disabled={anyRunning || !!busy} onChange={event => setPorts({ ...ports, [key]: Number(event.target.value) })} /></label>)}</div>
              <details className="advanced"><summary>Advanced settings</summary>
                <label><span>PHP version<small>For websites, CLI PHP, and Composer. PHP 8.5 is the default.</small></span><select value={phpVersion} disabled={!canControl || !!busy || anyRunning || !data.runtimeReady} onChange={event => setPhpVersion(event.target.value)}>{Object.keys(data.phpVersions).sort().map(version => <option key={version} value={version}>PHP {version}{['8.0', '8.1'].includes(version) ? ' · Legacy' : ''}</option>)}</select></label>
                <label><span>PHP FastCGI port<small>Internal connection between Nginx and PHP</small></span><input type="number" min="1024" max="65535" required value={ports.php} disabled={anyRunning || !!busy} onChange={event => setPorts({ ...ports, php: Number(event.target.value) })} /></label>
                <p className="settings-hint">PHP 8.0 and 8.1 are unsupported legacy versions for older coursework. Use PHP 8.5 for new projects.</p>
              </details>
              {anyRunning && <p className="settings-hint">Services are running. Stop all from Environment to edit settings.</p>}
              <button className="primary" disabled={!canControl || anyRunning || !!busy || !data.runtimeReady} type="submit">Save</button>
            </form>
          </section>
          <section className="settings-card cli-tools" aria-label="Command-line tools">
            <div className="card-title"><h2>Command-line tools</h2><div className="cli-title-actions"><HelpTip label="Command-line tools help"><p>Open a new terminal or restart VS Code after enabling these tools.</p><p>Check with <code>php --version</code>, <code>composer --version</code>{databaseCliAvailable && <>, and <code>fairway db --help</code></>}.</p>{databaseCliAvailable && <p>Database tools support SQL export/import. Start MySQL for database operations. If fairway is missing after an upgrade, disable and enable these tools once.</p>}<p>App detection can differ from your terminal. “Open terminal” uses Fairway XMP’s commands.</p></HelpTip><span className={`status-label ${data.pathInstalled ? 'ready' : ''}`}>{data.pathInstalled ? 'Added to PATH' : 'Not added to PATH'}</span></div></div>
            <p>{data.pathInstalled ? 'Available in new terminals and restarted editors.' : 'Add these commands to your user PATH.'}</p>
            <div className="cli-commands" aria-label="Terminal commands"><code>php</code><code>composer</code>{databaseCliAvailable && <code>fairway db</code>}</div>
            <button className="secondary" disabled={!canControl || !!busy || (!data.pathInstalled && !data.runtimeReady)} onClick={() => { void run('Updating user PATH…', async () => {
              const enabled = !data.pathInstalled;
              setData(await invoke<Snapshot>('cli_integration', { enabled }));
              setNotice({ kind: 'success', text: enabled ? `${cliTools} added to your user PATH. Open a new terminal or restart VS Code.` : 'Fairway XMP’s commands removed from your user PATH. Open a new terminal or restart VS Code.' });
            }); }}>{data.pathInstalled ? 'Remove from user PATH' : 'Enable command-line tools'}</button>
            <details className="cli-diagnostics"><summary>Command details</summary>
              {data.pathInstalled && <dl className="path-list cli-paths"><div><dt>PHP command</dt><dd>{data.cliPhp ?? 'Command location unavailable. Enable command-line tools again.'}</dd></div><div><dt>Composer command</dt><dd>{data.cliComposer ?? 'Command location unavailable. Enable command-line tools again.'}</dd></div>{databaseCliAvailable && <div><dt>Database commands</dt><dd>fairway db</dd></div>}</dl>}
              <dl className="path-list"><div><dt>PHP detected by this app</dt><dd>{data.pathPhp ?? 'Not detected in the app’s PATH'}</dd></div><div><dt>Composer detected by this app</dt><dd>{data.pathComposer ?? 'Not detected in the app’s PATH'}</dd></div></dl>
            </details>
          </section>
          <section className="settings-card"><h2>Help and diagnostics</h2><p>Create a support report with your app version, operating system, selected PHP, ports, service status, and command detection. It excludes passwords, database contents, project files, logs, and file paths.</p><button className="secondary" disabled={!canControl || !!busy} onClick={() => { void run('Preparing diagnostic report…', async () => setReport(await invoke<string>('diagnostic_report'))); }}>Diagnostic report<Clipboard size={14} /></button></section>
          <section className="settings-card"><h2>Your files</h2><dl className="path-list"><div><dt>Document root</dt><dd>{data.documentRoot}</dd></div><div><dt>Settings and database data</dt><dd>{data.dataDir}</dd></div></dl><div className="button-row"><button className="secondary" disabled={!canControl || !!busy} onClick={() => open('logs')}><ScrollText size={15} />Open logs</button><button className="secondary" disabled={!canControl || !!busy} onClick={() => open('backups')}><FolderOpen size={15} />Open backups</button></div></section>
        </>}

        {view === 'updates' && <>
          <div className="page-heading"><div><h1>A dependable environment.</h1><p>Find new versions of Fairway XMP and your bundled tools.</p></div></div>
          <section className="settings-card"><div className="card-title"><h2>Fairway XMP {data.version}</h2><span className="status-label">Installed version</span></div><div className="runtime-list">{['php', 'nginx', 'mysql', 'phpmyadmin', 'composer'].map(name => <div key={name}><span>{name === 'php' ? 'PHP' : name === 'mysql' ? 'MySQL' : name === 'phpmyadmin' ? 'phpMyAdmin' : name === 'nginx' ? 'Nginx' : 'Composer'}</span><code>{name === 'php' ? selectedPhp ?? 'Unavailable' : versions?.[name] ?? 'Unavailable'}</code></div>)}</div><p>{update?.configured ? 'Updates preserve your projects and database data. Fairway XMP backs up student databases and stops services before installing an update.' : 'Download a newer installer from View downloads. Back up your databases, stop all services, and quit Fairway XMP before installing. Your projects and database data are kept.'}</p><div className="button-row update-actions"><button className="primary" disabled={!canControl || !!busy} onClick={() => { void run('Checking for updates…', async () => {
            const result = await invoke<UpdateInfo>('check_update'); setUpdate(result);
            if (result.configured && !result.version) setNotice({ kind: 'success', text: 'You’re using the latest available release.' });
            if (!result.configured) setNotice({ kind: 'success', text: 'This version uses manual updates. Choose View downloads to find a newer installer.' });
          }); }}><RefreshCw size={15} />Check for updates</button>
            <button className="secondary" disabled={!canControl || !!busy} onClick={() => open('releases')}>View downloads<ExternalLink size={15} /></button><button className="secondary" disabled={!canControl || !!busy} onClick={() => open('source')}>Source code<ExternalLink size={15} /></button><button className="secondary" disabled={!canControl || !!busy} onClick={() => open('issues')}>Get help<ExternalLink size={15} /></button></div>
            {update && !update.configured && <p className="settings-hint">Visit the downloads page to see available releases.</p>}
            {update?.version && <div className="update-available"><h3>Version {update.version} is available</h3><p className="release-notes">{update.notes ?? 'A tested environment update is ready.'}</p><button className="primary" disabled={!!busy} onClick={() => { void run('Backing up and installing update…', () => invoke<void>('install_update')); }}>Install update</button></div>}
          </section>
        </>}
      </div>
      {busy && !databaseBusy && <div className="operation-status" role="status"><LoaderCircle className="spin" size={16} />{busy}</div>}
    </main>

    {report !== null && <Modal title="Diagnostic report" onClose={() => setReport(null)}><p>Copy this report when asking for help. You can review everything included below.</p><pre className="log-output">{report}</pre><button className="secondary" onClick={() => { void copy(report, 'diagnostics'); }}>{copied === 'diagnostics' ? <Check size={14} /> : <Clipboard size={14} />}{copied === 'diagnostics' ? 'Copied' : 'Copy diagnostic report'}</button></Modal>}
    {log && <Modal title={`${meta[log.service].name} logs`} onClose={() => setLog(null)}><pre className="log-output">{log.text}</pre><button className="secondary" disabled={!!busy} onClick={() => showLog(log.service)}><RefreshCw size={15} />Refresh log</button></Modal>}
    {help && <Modal title="Your first PHP project" onClose={() => setHelp(false)}><ol className="help-steps"><li>Click <strong>Start all</strong> and wait for the services to become ready.</li><li>Open your coursework folder: <code>fairway/www</code>.</li><li>Create a folder called <code>lesson-01</code> and add an <code>index.php</code> file.</li><li>Open <code>http://127.0.0.1:{data.ports.website}/lesson-01/</code> in your browser.</li></ol><p>Log into phpMyAdmin with username <code>fairway</code> and password <code>fairway</code>. Reopen your terminal after installation to use PHP and Composer. If they are unavailable, enable them in Settings.</p><p className="settings-hint">Closing this window keeps services running. Use the Fairway XMP menu bar or system tray icon to reopen the panel, control services, or quit. Quitting stops all services.</p></Modal>}
    {projectForm && <Modal title={projectForm.previous ? 'Edit project' : 'Add project'} onClose={() => setProjectForm(null)} backdropCloses={false}><form className="project-form" onSubmit={event => { event.preventDefault(); saveProject(projectForm); }}>
      <label><span>Name</span><input value={projectForm.name} maxLength={80} required onChange={event => { const name = event.target.value; setProjectForm({ ...projectForm, name, domain: projectForm.domainEdited ? projectForm.domain : suggestDomain(name) }); }} /></label>
      <label><span>Domain</span><span className="domain-input"><input value={projectForm.domain} maxLength={63} required pattern="[a-z0-9]([a-z0-9-]*[a-z0-9])?" onChange={event => setProjectForm({ ...projectForm, domain: event.target.value.toLowerCase(), domainEdited: true })} /><span>.test</span></span><small>Lowercase letters, numbers, and hyphens.</small></label>
      <label><span>Web root</span><span className="root-input"><input value={projectForm.root} readOnly required placeholder="Choose a folder" aria-label="Web root folder" className="root-path" onClick={() => { if (!busy) void chooseRoot(projectForm); }} onKeyDown={event => { if ((event.key === 'Enter' || event.key === ' ') && !busy) { event.preventDefault(); void chooseRoot(projectForm); } }} /><button type="button" className="secondary" disabled={!!busy} onClick={() => { void chooseRoot(projectForm); }}><FolderOpen size={14} />Choose…</button></span><small>The folder Fairway serves. For Laravel, choose the project’s <code>public</code> folder; files outside the web root stay private. Folders inside your document root are also served at <code>127.0.0.1:{data.ports.website}</code>.</small></label>
      <label className="project-check"><input type="checkbox" checked={projectForm.indexFallback} onChange={event => setProjectForm({ ...projectForm, indexFallback: event.target.checked })} /><span>Route missing paths to <code>index.php</code><small>Turn on for Laravel and other frameworks, so routes like <code>/about</code> work.</small></span></label>
      {projectForm.previous !== projectForm.domain && <p className="settings-hint">Saving {projectForm.previous ? `replaces ${projectForm.previous}.test with` : 'adds'} <code>{projectForm.domain || 'your-domain'}.test</code> in your computer’s hosts file, pointing to this computer. {hostsApproval[data.os] ?? 'Your computer will ask for administrator permission.'}</p>}
      {projectForm.error && <div className="notice error" role="alert"><span>{projectForm.error}</span></div>}
      <div className="button-row domain-confirm-actions"><button type="button" className="secondary" onClick={() => setProjectForm(null)}>Cancel</button><button type="submit" className="primary" disabled={!!busy || !projectForm.root}>{projectForm.previous ? 'Save changes' : 'Add project'}</button></div>
    </form></Modal>}
    {removing && <Modal title={`Remove ${removing.name}?`} onClose={() => setRemoving(null)}><p>Fairway stops serving <code>{removing.domain}.test</code>{removing.domainReady && <> and removes it from your computer’s hosts file. {hostsApproval[data.os] ?? 'Your computer will ask for administrator permission.'}</>}{!removing.domainReady && '.'} The folder <code>{removing.root}</code> and its files are kept.</p>{data.httpsTrusted && !data.projects.some(project => project.https && project.domain !== removing.domain) && <p>No other project uses HTTPS, so Fairway also removes its local certificate trust. Your computer may ask you to confirm.</p>}<div className="button-row domain-confirm-actions"><button className="secondary" onClick={() => setRemoving(null)}>Cancel</button><button className="primary" onClick={() => { const { domain, name } = removing; setRemoving(null); void run('Removing project…', async () => { setData(await invoke<Snapshot>('remove_project', { domain })); setNotice({ kind: 'success', text: `${name} was removed. Its files were kept.` }); }); }}>Remove project</button></div></Modal>}
    {sslSetup && <Modal title={`Enable HTTPS for ${sslSetup.domain}.test?`} onClose={() => setSslSetup(null)}><p>HTTPS uses a local certificate that only this computer trusts. {trustApproval[data.os] ?? 'Your computer will ask you to approve trusting the certificate.'}</p><p>The certificate stays trusted until you remove it from this list or uninstall Fairway. <code>http://{sslSetup.domain}.test:{data.ports.website}/</code> keeps working.</p><div className="button-row domain-confirm-actions"><button className="secondary" onClick={() => setSslSetup(null)}>Cancel</button><button className="primary" onClick={() => { const project = sslSetup; setSslSetup(null); setHttps(project, true); }}>Enable HTTPS</button></div></Modal>}
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
    <div className="service-actions">{browser ? <button className="service-start" disabled={!service.ready || busy} onClick={() => onOpen('phpmyadmin')}>Open phpMyAdmin<ArrowUpRight size={14} /></button> : service.id === 'php' ? <span className="service-managed">Managed with Nginx</span> : <><button className="service-start" disabled={disabled} onClick={() => onAction(service.running ? 'stop' : 'start', service.id)}>{service.running ? <Square size={12} /> : <Play size={12} />}{service.running ? 'Stop service' : 'Start service'}</button><button className="icon-button" title={`Restart ${item.name}`} aria-label={`Restart ${item.name}`} disabled={disabled || !service.running} onClick={() => onAction('restart', service.id)}><RefreshCw size={14} /></button></>}<button className="icon-button" title={`View ${item.name} logs`} aria-label={`View ${item.name} logs`} disabled={!canControl || busy} onClick={() => onLog(service.id)}><ScrollText size={15} /></button></div>
  </section>;
}

function Modal({ title, onClose, children, backdropCloses = true }: { title: string; onClose: () => void; children: React.ReactNode; backdropCloses?: boolean }) {
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
  return <div className="modal-backdrop" onClick={event => { if (backdropCloses && event.target === event.currentTarget) onClose(); }}><div ref={container} className="modal" role="dialog" aria-modal="true" aria-label={title}><div className="card-title"><h2>{title}</h2><button className="icon-button" aria-label="Close dialog" onClick={onClose}><X size={20} /></button></div>{children}</div></div>;
}
