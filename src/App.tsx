import { Activity, lazy, Suspense, useCallback, useEffect, useRef, useState } from 'react';
import {
  ArrowDownToLine, ArrowUpRight, Check, ChevronRight, CircleHelp, Clipboard, Code2,
  AppWindow, Database, Download, ExternalLink, FolderOpen, Globe, Leaf, LoaderCircle, Mail, Play,
  RefreshCw, ScrollText, Settings2, Square, Terminal, X,
} from 'lucide-react';
import { canControl, invoke, isDesktop, isUiTest, listen, preview } from './bridge';
import type { Ports, Project, ProjectConfig, ServiceId, ServiceStatus, Snapshot, UpdateInfo, UpdateProgress } from './types';
import HelpTip from './HelpTip';
import DataBrowser from './data/DataBrowser';
import SitesPage from './SitesPage';
import MailPage from './MailPage';
import LogsPage from './LogsPage';
import { insideDocumentRoot, siteSaveMessage } from './sites';
import type { TerminalLaunch } from './TerminalPage';

const UiTestToolbar = isUiTest ? lazy(() => import('./ui-test/Toolbar')) : null;
const TerminalPage = lazy(() => import('./TerminalPage'));
const dataBrowserAvailable = isUiTest || isDesktop;

const meta = {
  nginx: { name: 'Nginx', description: 'Your local web server', icon: Globe, version: 'nginx', color: 'green' },
  php: { name: 'PHP', description: 'Managed automatically with Nginx', icon: Code2, version: 'php', color: 'purple' },
  mysql: { name: 'MySQL', description: 'Your local database', icon: Database, version: 'mysql', color: 'blue' },
  phpmyadmin: { name: 'phpMyAdmin', description: 'Manage tables and data', icon: Database, version: 'phpmyadmin', color: 'orange' },
  mail: { name: 'Mail', description: 'Catches emails your apps send', icon: Mail, version: 'mailpit', color: 'teal' },
} as const;

type View = 'environment' | 'projects' | 'mail' | 'settings' | 'updates' | 'data' | 'terminal' | 'logs';
const viewTitle: Record<View, string> = {
  environment: 'Environment',
  projects: 'Manage Sites',
  mail: 'View Mail',
  settings: 'Settings',
  updates: 'Updates',
  data: 'Browse Data',
  terminal: 'Terminal',
  logs: 'Logs',
};
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
  macos: 'macOS may ask you to approve address changes.',
  windows: 'Windows will ask for administrator permission.',
};
const trustApproval: Record<string, string> = {
  macos: 'Fairway adds its local certificate to your login keychain. macOS asks you to approve the trust setting; use Touch ID if offered, or your password. An administrator account is not required.',
  windows: 'Fairway adds its local certificate to your Windows user certificate store. Windows shows a security warning; choose Yes to continue. An administrator account is not required.',
};

export default function App() {
  const [data, setData] = useState<Snapshot>(preview);
  const [view, setCurrentView] = useState<View>('environment');
  const navigation = useRef({ view: 'environment' as View, id: 0 });
  const pageId = navigation.current.id;

  function setView(next: View) {
    if (navigation.current.view !== next) {
      navigation.current = { view: next, id: navigation.current.id + 1 };
      setNoticeState(null);
    }
    setCurrentView(next);
  }
  const [logRequest, setLogRequest] = useState<{ key: number; source: string } | null>(null);

  function showLog(service: ServiceId) {
    setLogRequest(current => ({ key: (current?.key ?? 0) + 1, source: service === 'phpmyadmin' ? 'nginx' : service }));
    setView('logs');
  }
  const [terminalOpened, setTerminalOpened] = useState(false);
  const [terminalFocus, setTerminalFocus] = useState(0);
  const [terminalLaunch, setTerminalLaunch] = useState<TerminalLaunch | null>(null);
  const [localBusy, setBusy] = useState<string | null>(null);
  const [backendBusy, setBackendBusy] = useState<string | null>(null);
  const busy = backendBusy ?? localBusy;
  const databaseBusy = backendBusy === 'Working with database…';
  const [notice, setNoticeState] = useState<Notice>(null);
  const lastBackendError = useRef<string | null>(null);
  function setNotice(next: Notice) {
    // An operation may finish after the user has left its page.
    if (navigation.current.id === pageId) setNoticeState(next);
  }
  const reportBackendError = useCallback((error: unknown) => {
    const text = message(error);
    if (lastBackendError.current === text) return;
    lastBackendError.current = text;
    setNoticeState({ kind: 'error', text });
  }, []);
  const [report, setReport] = useState<string | null>(null);
  const [help, setHelp] = useState(false);
  const [mailSettings, setMailSettings] = useState(false);
  const [ports, setPorts] = useState<Ports>(preview.ports);
  const [phpVersion, setPhpVersion] = useState(preview.phpVersion);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const [updateProgress, setUpdateProgress] = useState<UpdateProgress | null>(null);
  const [copied, setCopied] = useState<string | null>(null);
  const [projectForm, setProjectForm] = useState<ProjectForm | null>(null);
  const [removing, setRemoving] = useState<Project | null>(null);
  const [sslSetup, setSslSetup] = useState<Project | null>(null);
  const operation = useRef(false);
  const backendOperation = useRef(false);
  const operationFocus = useRef<{ trigger: HTMLElement; pageId: number } | null>(null);
  const versions = data.runtimeVersions;
  const selectedPhp = data.phpVersions[data.phpVersion] ?? versions?.php;
  const allReady = data.services.filter(service => service.id !== 'mail' || data.mailAvailable || service.running).every(service => service.ready);
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
        if (!disposed) reportBackendError(error);
      } finally { fetching = false; }
    };
    void poll();
    const unlisteners: (() => void)[] = [];
    void Promise.all([
      ...(isUiTest ? [listen('ui-test-reset', payload => {
        const current = payload as Snapshot;
        setPorts(current.ports); setPhpVersion(current.phpVersion);
        setNoticeState(null); lastBackendError.current = null;
        setUpdate(null); setUpdateProgress(null); setReport(null); setHelp(false); setCopied(null);
      })] : []),
      listen('fairway-status', payload => setData(payload as Snapshot)),
      listen('fairway-operation-started', payload => {
        backendOperation.current = true; setBackendBusy(String(payload));
      }),
      listen('fairway-operation-finished', () => {
        backendOperation.current = false; setBackendBusy(null);
        setUpdateProgress(null);
      }),
      listen('fairway-update-progress', payload => setUpdateProgress(payload as UpdateProgress)),
      listen('fairway-error', payload => {
        setBusy(null); operation.current = false;
        reportBackendError(String(payload));
      }),
      listen('fairway-shutdown', () => {
        backendOperation.current = true; setBackendBusy('Stopping services before quitting…');
      }),
    ]).then(items => {
      if (disposed) items.forEach(fn => fn()); else unlisteners.push(...items);
    }).catch(error => { if (!disposed) reportBackendError(error); });
    return () => { disposed = true; unlisteners.forEach(fn => fn()); };
  }, []);

  useEffect(() => { setPorts(data.ports); }, [data.ports.website, data.ports.phpmyadmin, data.ports.mysql, data.ports.php, data.ports.mail, data.ports.smtp]);
  useEffect(() => { setPhpVersion(data.phpVersion); }, [data.phpVersion]);
  useEffect(() => {
    if (busy || !operationFocus.current) return;
    const { trigger, pageId: origin } = operationFocus.current;
    operationFocus.current = null;
    // Some WebViews blur a disabled button. Wait until React has enabled it,
    // and never take focus from navigation, a modal, or another control.
    if (navigation.current.id === origin && document.activeElement === document.body &&
        !document.querySelector('[aria-modal="true"]') && trigger.isConnected && !trigger.matches(':disabled')) {
      trigger.focus({ preventScroll: true });
    }
  }, [busy]);

  async function run(label: string, action: () => Promise<void>) {
    if (operation.current || backendOperation.current) return;
    const trigger = document.activeElement instanceof HTMLElement ? document.activeElement : null;
    operationFocus.current = trigger ? { trigger, pageId } : null;
    operation.current = true;
    lastBackendError.current = null;
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
    if (target === 'mail') { setView('mail'); return; }
    if (target === 'terminal') { setTerminalOpened(true); setView('terminal'); setTerminalFocus(value => value + 1); return; }
    void run('Opening…', () => invoke<void>('open_target', { target }));
  }

  async function copy(text: string, key: string) {
    try {
      await navigator.clipboard.writeText(text);
      setCopied(key); window.setTimeout(() => setCopied(null), 1800);
    } catch (error) { setNotice({ kind: 'error', text: `Could not copy: ${message(error)}` }); }
  }

  function openSiteTerminal(site: Project, parent: boolean) {
    setTerminalLaunch({ key: crypto.randomUUID(), folder: `${parent ? 'site-parent' : 'site'}:${site.domain}`, label: `${site.name} · ${parent ? 'app folder' : 'web root'}` });
    setTerminalOpened(true); setView('terminal'); setTerminalFocus(value => value + 1);
  }

  function setHttps(project: Project, enabled: boolean) {
    void run('Updating site HTTPS…', async () => {
      const current = await invoke<Snapshot>('set_project_https', { domain: project.domain, enabled });
      setData(current);
      const saved = current.projects.find(site => site.domain === project.domain);
      const removedTrust = !enabled && data.httpsTrusted && !current.httpsTrusted;
      setNotice({ kind: 'success', text: enabled && saved ? siteSaveMessage(saved, current) : `HTTPS is off for ${project.domain}.test.${removedTrust ? ' Its unused certificate trust was removed.' : ''} HTTP remains enabled.` });
    });
  }

  function saveProject(form: ProjectForm) {
    const { previous, name, domain, root, https, indexFallback } = form;
    const project: ProjectConfig = { name: name.trim(), domain, root, https, indexFallback };
    void run('Saving site…', async () => {
      try {
        const current = await invoke<Snapshot>('save_project', { previous, project });
        setData(current); setProjectForm(null);
        const saved = current.projects.find(site => site.domain === project.domain);
        setNotice({ kind: 'success', text: saved ? siteSaveMessage(saved, current) : `${project.name} saved.` });
      } catch (error) {
        // A saved project with a pending hosts update is shown in the list with a retry.
        if (message(error).startsWith('The site was saved')) { setProjectForm(null); throw error; }
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

  const mailEnv = `MAIL_MAILER=smtp\nMAIL_HOST=127.0.0.1\nMAIL_PORT=${data.ports.smtp}\nMAIL_USERNAME=null\nMAIL_PASSWORD=null`;
  const connection = `DB_CONNECTION=mysql\nDB_HOST=127.0.0.1\nDB_PORT=${data.ports.mysql}\nDB_DATABASE=fairway\nDB_USERNAME=fairway\nDB_PASSWORD=fairway`;

  return <div className="app-shell">
    <aside className="sidebar">
      <a className="brand" href="#" onClick={event => { event.preventDefault(); setView('environment'); }} aria-label="Fairway XMP home">
        <span className="brand-mark"><Leaf size={23} strokeWidth={2.2} /></span><span className="brand-name">fairway<span className="brand-period">.</span><span className="brand-edition">XMP</span></span>
      </a>
      <nav className="sidebar-nav" aria-label="Main navigation">
        <div className="nav-group" role="group" aria-label="Core">
          <button className={view === 'environment' ? 'nav-item active' : 'nav-item'} onClick={() => setView('environment')}><Globe size={18} />Environment</button>
          <button className={view === 'terminal' ? 'nav-item active' : 'nav-item'} title="Terminal (Ctrl+`)" disabled={!!busy && !terminalOpened} onClick={() => open('terminal')}><Terminal size={18} />Terminal</button>
          <button className={view === 'logs' ? 'nav-item active' : 'nav-item'} onClick={() => setView('logs')}><ScrollText size={18} />Logs</button>
        </div>
        <div className="nav-group" role="group" aria-label="Tools">
          <button className={view === 'projects' ? 'nav-item active' : 'nav-item'} onClick={() => setView('projects')}><AppWindow size={18} />Manage Sites</button>
          <button className={view === 'mail' ? 'nav-item active' : 'nav-item'} onClick={() => setView('mail')}><Mail size={18} />View Mail</button>
          {dataBrowserAvailable && <button className={view === 'data' ? 'nav-item active' : 'nav-item'} disabled={!!busy && !databaseBusy} onClick={() => setView('data')}><Database size={18} />Browse Data</button>}
        </div>
        <div className="nav-group nav-group-app" role="group" aria-label="App">
          <button className={view === 'settings' ? 'nav-item active' : 'nav-item'} onClick={() => setView('settings')}><Settings2 size={18} />Settings</button>
          <button className={view === 'updates' ? 'nav-item active' : 'nav-item'} onClick={() => setView('updates')}><ArrowDownToLine size={18} />Updates</button>
        </div>
      </nav>
      <div className="sidebar-bottom">
        <button className="help-button" onClick={() => setHelp(true)}><CircleHelp size={17} />Getting started<ArrowUpRight size={14} /></button>
        <div className="sidebar-footer" aria-label="App version">v{data.version}</div>
      </div>
    </aside>

    <main>
      <header className="topbar"><span>Workspace <ChevronRight size={13} />{viewTitle[view]}</span><span className="local-pill"><span />Local environment</span></header>
      <div className={view === 'mail' || view === 'logs' ? 'main-content wide' : 'main-content'}>
        {UiTestToolbar && <Suspense fallback={null}><UiTestToolbar busy={!!busy} /></Suspense>}
        {!isDesktop && !isUiTest && <div className="preview-banner"><Code2 size={16} />Interface preview — open the desktop app to manage services.</div>}
        {terminalOpened && <div hidden={view !== 'terminal'}><Suspense fallback={<p>Loading terminal…</p>}><TerminalPage active={view === 'terminal'} busy={!!busy} focusRequest={terminalFocus} launch={terminalLaunch} /></Suspense></div>}
        {dataBrowserAvailable && <Activity mode={view === 'data' ? 'visible' : 'hidden'}><DataBrowser ready={data.services.some(service => service.id === 'mysql' && service.ready)} busy={!!busy && !databaseBusy} start={() => serviceAction('start', 'mysql')} open={open} /></Activity>}

        <div hidden={view !== 'logs'}><LogsPage active={view === 'logs'} services={data.services} projects={data.projects} request={logRequest} /></div>

        {view === 'environment' && <>
          <div className="page-heading"><div><h1>Ready when you are.</h1><p>Everything you need to build with PHP, in one place.</p></div><button className="primary" disabled={anyRunning ? !canControl || !!busy : !usable} onClick={() => serviceAction(anyRunning ? 'stop_all' : 'start_all')}>{busy ? <LoaderCircle className="spin" size={16} /> : anyRunning ? <Square size={14} /> : <Play size={15} fill="currentColor" />}{anyRunning ? 'Stop all' : 'Start all'}</button></div>
          <div className="section-heading"><h2>Services <span>5</span></h2><span className="stack-status"><span className={`status-dot ${allReady ? 'running' : ''}`} />{allReady ? 'All services ready' : anyRunning ? 'Environment partially running' : 'All services stopped'}</span></div>
          <div className="services-grid">{(['nginx', 'php', 'mysql', 'phpmyadmin', 'mail'] as ServiceId[]).map(id => {
            const service = data.services.find(s => s.id === id)!;
            return <ServiceCard key={id} service={service} unavailable={id === 'mail' && !data.mailAvailable && !service.running} version={id === 'php' ? selectedPhp : versions?.[meta[id].version]} disabled={!usable || (id === 'mail' && !data.mailAvailable && !service.running)} busy={!!busy} onAction={serviceAction} onOpen={open} onLog={showLog} />;
          })}</div>
          {canControl && !data.runtimeReady && <div className="notice error" role="alert">Fairway XMP’s PHP or database files are missing. Reinstall Fairway XMP to restore them. Your projects and database data will be kept.</div>}

          <div className="section-heading workspace-heading"><h2>Workspace details</h2></div>
          <div className="workspace-reference">
            <section className="reference-card document-root-card">
              <div className="card-title"><h2><FolderOpen size={17} />Document root</h2></div>
              <p className="reference-description">PHP files here open at http://127.0.0.1:{data.ports.website}/.</p>
              <code className="document-root-path">{data.documentRoot}</code>
              <div className="reference-actions"><button className="text-link" disabled={!canControl || !!busy} onClick={() => open('coursework')}><FolderOpen size={14} />Open folder<ArrowUpRight size={14} /></button></div>
            </section>
            <section className="reference-card connection-card"><div className="card-title"><h2><Database size={17} />Database connection</h2><button className="icon-button" title="Copy database environment settings" aria-label="Copy database environment settings" onClick={() => { void copy(connection, 'database'); }}>{copied === 'database' ? <Check size={16} /> : <Clipboard size={16} />}</button></div><dl><div><dt>Host</dt><dd>127.0.0.1<span className="port-inline">:{data.ports.mysql}</span></dd></div><div><dt>Database</dt><dd>fairway</dd></div><div><dt>Username / password</dt><dd>fairway / fairway</dd></div></dl><div className="reference-actions"><button className="text-link" disabled={!data.services.find(s => s.id === 'mysql')?.ready || !!busy} onClick={() => { void run('Backing up databases…', async () => { const path = await invoke<string>('backup_database'); setNotice({ kind: 'success', text: `Database backup saved: ${path}` }); }); }}><Download size={14} />Back up databases</button></div></section>
          </div>
          <footer className="content-footer"><span><span className="status-dot running" />127.0.0.1 · Local access only</span><button onClick={() => setView('settings')}>Configure environment<Settings2 size={13} /></button></footer>
        </>}

        {view === 'projects' && <SitesPage data={data} busy={!!busy} copied={copied}
          add={() => setProjectForm({ previous: null, name: '', domain: '', root: '', https: false, indexFallback: false, domainEdited: false, error: null })}
          edit={site => setProjectForm({ ...site, previous: site.domain, domainEdited: true, error: null })}
          remove={setRemoving} open={open} copy={(url, key) => { void copy(url, key); }} terminal={openSiteTerminal}
          start={() => serviceAction('start', 'nginx')} logs={() => showLog('nginx')}
          retry={() => { void run('Loading sites…', async () => { setData(await invoke<Snapshot>('snapshot')); }); }}
          sync={() => { void run('Setting up site addresses…', async () => { setData(await invoke<Snapshot>('sync_domains')); setNotice({ kind: 'success', text: 'Site addresses are set up.' }); }); }}
          https={(site, enabled) => { if (enabled && !data.httpsTrusted) setSslSetup(site); else setHttps(site, enabled); }}
          removeTrust={() => { void run('Removing certificate trust…', async () => { setData(await invoke<Snapshot>('remove_https_trust')); setNotice({ kind: 'success', text: 'Fairway’s local certificate is no longer trusted.' }); }); }}
        />}

        {view === 'mail' && <MailPage data={data} busy={!!busy} settings={() => setMailSettings(true)}
          start={() => serviceAction('start', 'mail')} stop={() => serviceAction('stop', 'mail')} logs={() => showLog('mail')}
          open={() => { void run('Opening…', () => invoke<void>('open_target', { target: 'mail' })); }} />}

        {view === 'settings' && <>
          <div className="page-heading"><div><h1>Environment settings.</h1><p>Predictable defaults, with room to adjust.</p></div></div>
          <section className="settings-card"><h2>Service Settings</h2><p>Stop all services before changing ports or PHP. Saved settings stay the same across restarts.</p>
            <form onSubmit={event => { event.preventDefault(); void run('Saving settings…', async () => {
              const mysqlPortChanged = ports.mysql !== data.ports.mysql;
              setData(await invoke<Snapshot>('save_settings', { ports, version: phpVersion })); setNotice({ kind: 'success', text: mysqlPortChanged ? 'Settings saved. Update existing project connection settings for the new MySQL port.' : 'Settings saved.' });
            }); }}>
              <div className="port-fields">{([['website', 'Website', 'Nginx'], ['phpmyadmin', 'phpMyAdmin', 'Database browser'], ['mysql', 'MySQL', 'Database connection'], ['mail', 'Mail inbox', 'Caught emails'], ['smtp', 'Mail SMTP', 'Where apps send email']] as const).map(([key, label, hint]) => <label key={key}><span>{label}<small>{hint}</small></span><input type="number" min="1024" max="65535" required value={ports[key]} disabled={anyRunning || !!busy} onChange={event => setPorts({ ...ports, [key]: Number(event.target.value) })} /></label>)}</div>
              <details className="advanced"><summary>Advanced settings</summary>
                <label><span>PHP version<small>For websites, CLI PHP, and Composer. PHP 8.5 is the default.</small></span><select value={phpVersion} disabled={!canControl || !!busy || anyRunning || !data.runtimeReady} onChange={event => setPhpVersion(event.target.value)}>{Object.keys(data.phpVersions).sort().map(version => <option key={version} value={version}>PHP {version}{['8.0', '8.1'].includes(version) ? ' · Legacy' : ''}</option>)}</select></label>
                <label><span>PHP FastCGI port<small>Internal connection between Nginx and PHP</small></span><input type="number" min="1024" max="65535" required value={ports.php} disabled={anyRunning || !!busy} onChange={event => setPorts({ ...ports, php: Number(event.target.value) })} /></label>
                <p className="settings-hint">PHP 8.0 and 8.1 are unsupported legacy versions for older projects. Use PHP 8.5 for new projects.</p>
              </details>
              {anyRunning && <p className="settings-hint">Services are running. Stop all from Environment to edit settings.</p>}
              <button className="primary" disabled={!canControl || anyRunning || !!busy || !data.runtimeReady} type="submit">Save</button>
            </form>
          </section>
          <section className="settings-card cli-tools" aria-label="Command-line tools">
            <div className="card-title"><h2>Command-line tools</h2><div className="cli-title-actions"><HelpTip label="Command-line tools help"><p>Open a new terminal or restart VS Code after enabling these tools.</p><p>Check with <code>php --version</code>, <code>composer --version</code>{databaseCliAvailable && <>, and <code>fairway db --help</code></>}.</p>{databaseCliAvailable && <p>Database tools support SQL export/import. Start MySQL for database operations. If fairway is missing after an upgrade, disable and enable these tools once.</p>}<p>App detection can differ from your terminal. The embedded terminal uses Fairway XMP’s commands.</p></HelpTip><span className={`status-label ${data.pathInstalled ? 'ready' : ''}`}>{data.pathInstalled ? 'Added to PATH' : 'Not added to PATH'}</span></div></div>
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
          <section className="settings-card"><h2>Your files</h2><dl className="path-list"><div><dt>Document root</dt><dd>{data.documentRoot}</dd></div><div><dt>Settings and database data</dt><dd>{data.dataDir}</dd></div></dl><div className="button-row"><button className="secondary" disabled={!canControl || !!busy} onClick={() => open('logs')}><ScrollText size={15} />Open logs folder</button><button className="secondary" disabled={!canControl || !!busy} onClick={() => open('backups')}><FolderOpen size={15} />Open backups</button></div></section>
        </>}

        {view === 'updates' && <>
          <div className="page-heading"><div><h1>A dependable environment.</h1><p>Find new versions of Fairway XMP and your bundled tools.</p></div></div>
          <section className="settings-card"><div className="card-title"><h2>Fairway XMP {data.version}</h2><span className="status-label">Installed version</span></div><div className="runtime-list">{['php', 'nginx', 'mysql', 'phpmyadmin', 'composer', 'mailpit'].map(name => <div key={name}><span>{name === 'php' ? 'PHP' : name === 'mysql' ? 'MySQL' : name === 'phpmyadmin' ? 'phpMyAdmin' : name === 'nginx' ? 'Nginx' : name === 'mailpit' ? 'Mailpit' : 'Composer'}</span><code>{name === 'php' ? selectedPhp ?? 'Unavailable' : versions?.[name] ?? 'Unavailable'}</code></div>)}</div><p>{update?.configured === false ? 'Download a newer installer from View downloads. Back up your databases, stop all services, and quit Fairway XMP before installing. Your projects and database data are kept.' : 'Updates download and verify while your services keep running. Before installing, Fairway XMP backs up your databases, stops services and terminal sessions, then restarts. Your projects, saved sites, and settings are kept.'}</p><div className="button-row update-actions"><button className="primary" disabled={!canControl || !!busy} onClick={() => { void run('Checking for updates…', async () => {
            const result = await invoke<UpdateInfo>('check_update'); setUpdate(result);
            if (result.configured && !result.version) setNotice({ kind: 'success', text: 'You’re using the latest available release.' });
            if (!result.configured) setNotice({ kind: 'success', text: 'This version uses manual updates. Choose View downloads to find a newer installer.' });
          }); }}><RefreshCw size={15} />Check for updates</button>
            <button className="secondary" disabled={!canControl || !!busy} onClick={() => open('releases')}>View downloads<ExternalLink size={15} /></button><button className="secondary" disabled={!canControl || !!busy} onClick={() => open('source')}>Source code<ExternalLink size={15} /></button><button className="secondary" disabled={!canControl || !!busy} onClick={() => open('issues')}>Get help<ExternalLink size={15} /></button></div>
            {update && !update.configured && <p className="settings-hint">Visit the downloads page to see available releases.</p>}
            {updateProgress && <div className="update-progress" role="status"><span>{updateProgress.phase === 'downloading' ? `Downloading update${updateProgress.total ? ` · ${Math.min(100, Math.floor((updateProgress.downloaded ?? 0) / updateProgress.total * 100))}%` : ''}…` : updateProgress.phase === 'verifying' ? 'Verifying update…' : updateProgress.phase === 'preparing' ? 'Backing up databases and stopping services…' : 'Installing update and restarting…'}</span>{updateProgress.phase === 'downloading' && <progress aria-label="Update download" value={updateProgress.total ? updateProgress.downloaded ?? 0 : undefined} max={updateProgress.total ?? undefined} />}</div>}
            {update?.version && <div className="update-available"><h3>Version {update.version} is available</h3><p className="release-notes">{update.notes ?? 'A tested environment update is ready.'}</p><button className="primary" disabled={!!busy} onClick={() => { void run('Downloading update…', async () => { setUpdateProgress({ phase: 'downloading', downloaded: 0 }); try { await invoke<void>('install_update', { version: update.version }); } finally { setUpdateProgress(null); } }); }}>Install update</button></div>}
          </section>
        </>}
      </div>
      <div className={`operation-notice${notice ? ` notice ${notice.kind}` : ''}`}>
        <div className="operation-notice-message">
          <div role={notice?.kind === 'success' ? 'status' : undefined} aria-live="polite" aria-atomic="true">{notice?.kind === 'success' ? notice.text : null}</div>
          <div role={notice?.kind === 'error' ? 'alert' : undefined} aria-live="assertive" aria-atomic="true">{notice?.kind === 'error' ? notice.text : null}</div>
        </div>
        {notice && <button aria-label="Dismiss message" onClick={() => setNoticeState(null)}><X size={17} /></button>}
      </div>
      {busy && !databaseBusy && <div className="operation-status" role="status"><LoaderCircle className="spin" size={16} />{busy}</div>}
    </main>

    {report !== null && <Modal title="Diagnostic report" onClose={() => setReport(null)}><p>Copy this report when asking for help. You can review everything included below.</p><pre className="log-output">{report}</pre><button className="secondary" onClick={() => { void copy(report, 'diagnostics'); }}>{copied === 'diagnostics' ? <Check size={14} /> : <Clipboard size={14} />}{copied === 'diagnostics' ? 'Copied' : 'Copy diagnostic report'}</button></Modal>}
    {mailSettings && <Modal title="Mail connection" onClose={() => setMailSettings(false)}><p>PHP’s <code>mail()</code> already sends here. Frameworks that send through SMTP need these settings.</p><dl className="mail-settings"><div><dt>SMTP host</dt><dd>127.0.0.1</dd></div><div><dt>Port</dt><dd>{data.ports.smtp}</dd></div><div><dt>Username / password</dt><dd>Not required</dd></div></dl><div className="card-title mail-env-title"><h2>Laravel .env</h2><button className="secondary" aria-label="Copy Laravel mail settings" onClick={() => { void copy(mailEnv, 'mail-env'); }}>{copied === 'mail-env' ? <Check size={14} /> : <Clipboard size={14} />}{copied === 'mail-env' ? 'Copied' : 'Copy'}</button></div><pre className="log-output">{mailEnv}</pre></Modal>}
    {help && <Modal title="Your first PHP project" onClose={() => setHelp(false)}><ol className="help-steps"><li>Click <strong>Start all</strong> and wait for the services to become ready.</li><li>Open the default web folder: <code>fairway/www</code>.</li><li>Create a folder called <code>lesson-01</code> and add an <code>index.php</code> file.</li><li>Open <code>http://127.0.0.1:{data.ports.website}/lesson-01/</code> in your browser.</li></ol><p>Log into phpMyAdmin with username <code>fairway</code> and password <code>fairway</code>. Reopen your terminal after installation to use PHP and Composer. If they are unavailable, enable them in Settings.</p><p className="settings-hint">Closing this window keeps services running. Use the Fairway XMP menu bar or system tray icon to reopen the panel, control services, or quit. Quitting stops all services.</p></Modal>}
    {projectForm && <Modal title={projectForm.previous ? 'Edit site' : 'Add site'} onClose={() => setProjectForm(null)} backdropCloses={false}><form className="project-form modal-form" onSubmit={event => { event.preventDefault(); saveProject(projectForm); }}><div className="modal-form-body">
      <label><span>Site name</span><input aria-label="Site name" disabled={!!busy} value={projectForm.name} maxLength={80} required onChange={event => { const name = event.target.value; setProjectForm({ ...projectForm, name, domain: projectForm.domainEdited ? projectForm.domain : suggestDomain(name) }); }} /></label>
      <div className="site-field"><div className="site-field-title"><label htmlFor="site-domain">Domain</label><HelpTip label="About site domains"><p>Use lowercase letters, numbers and hyphens. The site name can use any language.</p><p>Your address ends in <code>.test</code> and works only on this computer.</p></HelpTip></div><span className="domain-input"><input id="site-domain" aria-label="Domain" disabled={!!busy} value={projectForm.domain} maxLength={63} required pattern="[a-z0-9]([a-z0-9-]*[a-z0-9])?" onChange={event => setProjectForm({ ...projectForm, domain: event.target.value.toLowerCase(), domainEdited: true })} /><span>.test</span></span></div>
      <div className="site-field"><div className="site-field-title"><label htmlFor="site-root">Web root</label><HelpTip label="About the web root"><p>Only this folder is served at the site’s address. For Laravel, choose the app’s <code>public</code> folder.</p></HelpTip></div><span className="root-input"><input id="site-root" value={projectForm.root} readOnly required placeholder="Choose a folder" aria-label="Web root folder" className="root-path" onClick={() => { if (!busy) void chooseRoot(projectForm); }} onKeyDown={event => { if ((event.key === 'Enter' || event.key === ' ') && !busy) { event.preventDefault(); void chooseRoot(projectForm); } }} /><button type="button" className="secondary" disabled={!!busy} onClick={() => { void chooseRoot(projectForm); }}><FolderOpen size={14} />Choose…</button></span></div>
      {insideDocumentRoot(projectForm.root, data.documentRoot, data.os) && <div className="settings-hint site-inline-help"><span>Also available through localhost.</span><HelpTip label="About shared document-root access" placement="above"><p>This folder is inside <code>{data.documentRoot}</code>. Other project files may be accessible at <code>http://127.0.0.1:{data.ports.website}/</code>. Keep apps with private files outside it.</p></HelpTip></div>}
      <div className="site-field"><div className="site-field-title"><label htmlFor="site-type">App type</label><HelpTip label="About app routing" placement="above"><p>Basic PHP / HTML serves files directly. Laravel / front controller routes missing paths to <code>index.php</code>.</p><p>For Laravel, choose its <code>public</code> folder as the web root.</p></HelpTip></div><select id="site-type" aria-label="App type" disabled={!!busy} value={projectForm.indexFallback ? 'front-controller' : 'files'} onChange={event => setProjectForm({ ...projectForm, indexFallback: event.target.value === 'front-controller' })}><option value="files">Basic PHP / HTML</option><option value="front-controller">Laravel / front controller</option></select></div>
      {projectForm.previous !== projectForm.domain && <div className="settings-hint site-inline-help"><span>Address changes may need system approval.</span><HelpTip label="About address permissions" placement="above"><p>Saving {projectForm.previous ? `replaces ${projectForm.previous}.test with` : 'adds'} <code>{projectForm.domain || 'your-domain'}.test</code> in your computer’s hosts file, pointing to this computer.</p><p>{hostsApproval[data.os] ?? 'Your computer will ask for administrator permission.'}</p></HelpTip></div>}
      {projectForm.error && <div className="notice error" role="alert"><span>{projectForm.error}</span></div>}
      </div><div className="button-row domain-confirm-actions modal-form-actions"><button type="button" className="secondary" onClick={() => setProjectForm(null)}>Cancel</button><button type="submit" className="primary" disabled={!!busy || !projectForm.root}>{projectForm.previous ? 'Save changes' : 'Add site'}</button></div>
    </form></Modal>}
    {removing && <Modal title={`Remove ${removing.name}?`} onClose={() => setRemoving(null)}><p>Fairway stops serving <code>{removing.domain}.test</code>{removing.domainReady && <> and removes it from your computer’s hosts file. {hostsApproval[data.os] ?? 'Your computer will ask for administrator permission.'}</>}{!removing.domainReady && '.'} The folder <code>{removing.root}</code> and its files are kept.</p>{data.httpsTrusted && !data.projects.some(project => project.https && project.domain !== removing.domain) && <p>No other site uses HTTPS, so Fairway also removes its local certificate trust. Your computer may ask you to confirm.</p>}<div className="button-row domain-confirm-actions"><button className="secondary" onClick={() => setRemoving(null)}>Cancel</button><button className="primary" onClick={() => { const { domain, name } = removing; setRemoving(null); void run('Removing site…', async () => { setData(await invoke<Snapshot>('remove_project', { domain })); setNotice({ kind: 'success', text: `${name} was removed. Its files were kept.` }); }); }}>Remove site</button></div></Modal>}
    {sslSetup && <Modal title={`Enable HTTPS for ${sslSetup.domain}.test?`} onClose={() => setSslSetup(null)}><p>HTTPS uses a local certificate that only this computer trusts. {trustApproval[data.os] ?? 'Your computer will ask you to approve trusting the certificate.'}</p><p>The site will use <code>https://{sslSetup.domain}.test:8443/</code>. HTTP remains enabled. Turning off HTTPS for the last site also removes the shared certificate trust. Your computer may ask you to confirm.</p><div className="button-row domain-confirm-actions"><button className="secondary" onClick={() => setSslSetup(null)}>Cancel</button><button className="primary" onClick={() => { const project = sslSetup; setSslSetup(null); setHttps(project, true); }}>Enable HTTPS</button></div></Modal>}
  </div>;
}

function ServiceCard({ service, version, unavailable, disabled, busy, onAction, onOpen, onLog }: {
  service: ServiceStatus; version?: string; unavailable: boolean; disabled: boolean; busy: boolean;
  onAction: (action: string, service?: ServiceId) => void;
  onOpen: (target: string) => void; onLog: (service: ServiceId) => void;
}) {
  const item = meta[service.id];
  const Icon = item.icon;
  const browser = service.id === 'phpmyadmin';
  const web = browser || service.id === 'nginx' || service.id === 'mail';
  const status = <span className={`status-label ${service.ready ? 'ready' : service.running ? 'pending' : ''}`}><span className={`status-dot ${service.ready ? 'running' : service.running ? 'pending' : ''}`} />{service.ready ? 'Running' : service.running ? 'Waiting' : 'Stopped'}</span>;
  const address = <div className="service-address"><span>{service.id === 'php' ? 'Internal port' : web ? 'localhost' : '127.0.0.1'}<span className="address-port">:{service.port}</span></span>{web && <button className="icon-button" title={`Open ${item.name}`} aria-label={`Open ${item.name}`} disabled={!service.ready || busy} onClick={() => onOpen(service.id === 'nginx' ? 'website' : service.id)}><ExternalLink size={14} /></button>}</div>;
  const error = (service.message || unavailable) && <div className="service-error" role="status">{service.message || 'Mail catcher files are missing. Reinstall Fairway XMP to restore mail.'}</div>;
  const actions = <div className="service-actions">{browser ? <button className="service-start" disabled={!service.ready || busy} onClick={() => onOpen('phpmyadmin')}>Open phpMyAdmin<ArrowUpRight size={14} /></button> : service.id === 'php' ? <span className="service-managed">Managed with Nginx</span> : <><button className="service-start" disabled={disabled} onClick={() => onAction(service.running ? 'stop' : 'start', service.id)}>{service.running ? <Square size={12} /> : <Play size={12} />}{service.running ? 'Stop service' : 'Start service'}</button><button className="icon-button" title={`Restart ${item.name}`} aria-label={`Restart ${item.name}`} disabled={disabled || !service.running} onClick={() => onAction('restart', service.id)}><RefreshCw size={14} /></button></>}<button className="icon-button" title={`View ${item.name} logs`} aria-label={`View ${item.name} logs`} disabled={!canControl} onClick={() => onLog(service.id)}><ScrollText size={15} /></button></div>;

  if (service.id === 'mail') return <section className="service-card service-card-mail">
    <div className="mail-service-summary">
      <span className={`service-icon ${item.color}`}><Icon size={23} strokeWidth={1.8} /></span>
      <div className="mail-service-details">
        <div className="mail-service-heading"><div className="service-name"><h3>{item.name}</h3>{version && <span>{version}</span>}</div>{status}</div>
        <p>{item.description}</p>
      </div>
    </div>
    {address}{actions}{error}
  </section>;

  return <section className="service-card">
    <div className="service-top"><span className={`service-icon ${item.color}`}><Icon size={23} strokeWidth={1.8} /></span>{status}</div>
    <div className="service-name"><h3>{item.name}</h3>{version && <span>{version}</span>}</div><p>{item.description}</p>
    {address}{error}{actions}
  </section>;
}

function Modal({ title, onClose, children, backdropCloses = true }: { title: string; onClose: () => void; children: React.ReactNode; backdropCloses?: boolean }) {
  const container = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  useEffect(() => { close.current = onClose; }, [onClose]);
  useEffect(() => {
    const previous = document.activeElement as HTMLElement | null;
    const first = container.current?.querySelector<HTMLElement>('form input:not(:disabled), button');
    const field = container.current?.querySelector<HTMLInputElement>('form input:not(:disabled)');
    (field ?? first)?.focus();
    function key(event: KeyboardEvent) {
      if (event.key === 'Escape') close.current();
      if (event.key === 'Tab') {
        const focusable = container.current?.querySelectorAll<HTMLElement>('button:not(:disabled), a, input:not(:disabled), select:not(:disabled)');
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
