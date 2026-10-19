import { preview } from '../bridge';
import { terminalRequest } from './terminal';
import type { MailMessage, Ports, ProjectConfig, ServiceId, Snapshot } from '../types';
import { cancelData, databaseRequest, resetData } from './data-browser';

// Fixture modules own event subscriptions and state. Reload them together so
// Fast Refresh cannot keep a screen subscribed to an obsolete simulation.
if (import.meta.hot) import.meta.hot.accept(() => window.location.reload());

export const scenarios = ['Stopped', 'No sites', 'Saved sites unavailable', 'Hosts update pending', 'Missing web root', 'Running', 'Partially running', 'Port conflict', 'Missing runtime', 'Update available', 'Update failure', 'CLI not detected by app', 'Other PHP on PATH', 'CLI disabled', 'Logs under load', 'Logs empty', 'Logs unreadable', 'Slow mail', 'Mail unavailable'] as const;
export type Scenario = typeof scenarios[number];
export type Platform = 'Windows' | 'macOS';
const events = new EventTarget();
let scenario: Scenario = 'Stopped';
export const defaultPlatform: Platform = navigator.platform.startsWith('Mac') ? 'macOS' : 'Windows';
let platform: Platform = defaultPlatform;
let data: Snapshot;
let mailbox: (MailMessage & { Read: boolean })[] = [];

function seedMail() {
  const from = { Name: 'My Shop', Address: 'orders@shop.test' };
  const to = [{ Name: 'Student May', Address: 'may@example.com' }];
  const at = (minutes: number) => new Date(Date.now() - minutes * 60000).toISOString();
  mailbox = [
    { ID: 'welcome', From: from, To: to, Cc: null, Subject: 'Welcome to My Shop', Date: at(2), Read: false, Attachments: [], Text: 'Thanks for joining My Shop.', HTML: '<h1 style="color:#1e654c">Welcome!</h1><p>Thanks for joining <strong>My Shop</strong>.</p><script>document.body.textContent = "scripts must not run"</script>' },
    { ID: 'reset', From: from, To: to, Cc: null, Subject: 'Reset your password', Date: at(65), Read: true, Attachments: [], Text: 'Use this link to reset your password: http://shop.test:8800/reset/abc', HTML: '' },
    { ID: 'invoice', From: { Name: '', Address: 'billing@shop.test' }, To: to, Cc: [{ Name: '', Address: 'accounts@shop.test' }], Subject: 'Invoice #1042', Date: at(60 * 26), Read: true, Attachments: [{ PartID: '2', FileName: 'invoice-1042.pdf', ContentType: 'application/pdf', Size: 48213 }], Text: 'Your invoice is attached.', HTML: '<p>Your invoice is attached.</p>' },
  ];
}

function mailRequest(request: { action: string; query?: string; id?: string; ids?: string[] }) {
  if (!data.services.find(service => service.id === 'mail')?.ready) throw new Error('Mail is not running. Start mail to see caught emails.');
  if (request.action === 'list') {
    const query = (request.query ?? '').trim().toLowerCase();
    const found = mailbox.filter(item => !query || `${item.Subject} ${item.From?.Address} ${item.Text}`.toLowerCase().includes(query));
    return {
      messages_count: found.length, messages_unread: found.filter(item => !item.Read).length,
      messages: found.map(({ ID, Read, From, To, Subject, Date: Created, Text, Attachments }) => ({ ID, Read, From, To, Subject, Created, Snippet: Text.slice(0, 80), Attachments: Attachments?.length ?? 0 })),
    };
  }
  if (request.action === 'message') {
    const item = mailbox.find(entry => entry.ID === request.id);
    if (!item) throw new Error('This email is no longer available.');
    item.Read = true;
    return item;
  }
  if (request.action === 'delete') { mailbox = mailbox.filter(item => !request.ids?.includes(item.ID)); return null; }
  if (request.action === 'clear') { mailbox = []; return null; }
  throw new Error('Unsupported simulated mail request.');
}

function emit(event: string, payload: unknown) {
  events.dispatchEvent(new CustomEvent(event, { detail: payload }));
}

function paths() {
  const home = platform === 'Windows' ? 'C:\\Users\\Student May\\' : '/Users/Student May/';
  return {
    documentRoot: `${home}${platform === 'Windows' ? 'fairway\\www' : 'fairway/www'}`,
    dataDir: `${home}${platform === 'Windows' ? 'AppData\\Local\\dev.fairway.environment' : 'Library/Application Support/dev.fairway.environment'}`,
  };
}

function joinPath(...parts: string[]) {
  return parts.join(platform === 'Windows' ? '\\' : '/');
}

function home() {
  return platform === 'Windows' ? 'C:\\Users\\Student May' : '/Users/Student May';
}

function syncDomains() {
  for (const project of data.projects) project.domainReady = true;
  data.domainsPending = false;
}

function configureCli(enabled: boolean) {
  data.pathInstalled = enabled;
  data.cliPhp = enabled ? joinPath(data.dataDir, 'bin', platform === 'Windows' ? 'php.cmd' : 'php') : null;
  data.cliComposer = enabled ? joinPath(data.dataDir, 'bin', platform === 'Windows' ? 'composer.cmd' : 'composer') : null;
  data.pathPhp = enabled ? data.cliPhp : null;
  data.pathComposer = enabled ? data.cliComposer : null;
  if (scenario === 'CLI not detected by app') data.pathPhp = data.pathComposer = null;
  if (scenario === 'Other PHP on PATH') {
    data.pathPhp = platform === 'Windows' ? 'C:\\Other PHP\\php.exe' : '/opt/homebrew/bin/php';
    data.pathComposer = platform === 'Windows' ? 'C:\\Other PHP\\composer.cmd' : '/opt/homebrew/bin/composer';
  }
}

function running(id: ServiceId, value: boolean) {
  const ids = id === 'nginx' ? ['nginx', 'php', 'phpmyadmin'] : [id];
  for (const service of data.services) {
    if (ids.includes(service.id)) Object.assign(service, { running: value, ready: value, pid: value ? 12345 : null, message: null });
  }
}

export function select(next: Scenario, os: Platform) {
  resetData();
  seedMail();
  scenario = next; platform = os;
  data = { ...structuredClone(preview), ...paths(), runtimeReady: next !== 'Missing runtime', mailAvailable: next !== 'Missing runtime' && next !== 'Mail unavailable', pathInstalled: true };
  data.sitesError = next === 'Saved sites unavailable' ? 'The saved site list is invalid: expected a JSON array.' : null;
  data.projects = next === 'No sites' || data.sitesError ? [] : [
    { name: 'My Shop', domain: 'shop', root: joinPath(home(), 'Code', 'shop', 'public'), https: true, indexFallback: true, available: next !== 'Missing web root', domainReady: true },
    { name: 'Blog', domain: 'blog', root: joinPath(data.documentRoot, 'blog'), https: false, indexFallback: false, available: true, domainReady: next !== 'Hosts update pending' },
  ];
  data.httpsTrusted = next !== 'No sites';
  data.domainsPending = next === 'Hosts update pending';
  data.os = platform === 'Windows' ? 'windows' : 'macos';
  configureCli(next !== 'CLI disabled');
  if (next === 'Running' || next === 'Partially running' || next === 'Slow mail' || next === 'Mail unavailable') running('nginx', true);
  if (next === 'Running' || next === 'Slow mail') { running('mysql', true); running('mail', true); }
  if (next === 'Mail unavailable') running('mysql', true);
  if (next === 'Partially running') {
    Object.assign(data.services.find(service => service.id === 'mysql')!, {
      running: true, pid: 12346, message: 'MySQL is starting. Check the logs if it does not become ready.',
    });
  }
  if (next === 'Port conflict') data.services[0].message = 'Port 8800 is already in use by another application. Stop it or choose a different website port in Settings.';
  emit('ui-test-reset', structuredClone(data));
  emit('fairway-status', structuredClone(data));
}

select('Stopped', defaultPlatform);

export function listen(event: string, callback: (payload: unknown) => void) {
  const handler = (value: Event) => callback((value as CustomEvent).detail);
  events.addEventListener(event, handler);
  return () => events.removeEventListener(event, handler);
}

export async function invoke<T>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  window.dispatchEvent(new CustomEvent('fairway-ui-command', { detail: command }));
  if (command.startsWith('terminal_')) return terminalRequest(command, args, data.documentRoot, data.phpVersions[data.phpVersion], data.runtimeReady, data.projects) as T;
  if (command === 'snapshot') return structuredClone(data) as T;
  if (command === 'save_mail_attachment') return `/Users/you/Downloads/${String(args.name)}` as T;
  if (command === 'mail_inbox') {
    const request = args.request as Parameters<typeof mailRequest>[0];
    const response = structuredClone(mailRequest(request));
    if (scenario === 'Slow mail') await new Promise(resolve => window.setTimeout(resolve,
      request.query === 'welcome' || request.id === 'welcome' ? 1400 : 30));
    return response as T;
  }
  if (command === 'cancel_data_browser') { cancelData(String(args.id)); return undefined as T; }
  if (command === 'data_browser') {
    if (!data.services.find(service => service.id === 'mysql')?.ready) throw new Error('Start MySQL before browsing data.');
    emit('fairway-operation-started', 'Working with database…');
    try { return structuredClone(await databaseRequest(String(args.id), args.request as Record<string, unknown>)) as T; }
    finally { emit('fairway-operation-finished', null); }
  }
  // Keep pending states visible for human review, and preserve the app's operation guard.
  await new Promise(resolve => window.setTimeout(resolve, 600));
  let result: unknown;
  switch (command) {
    case 'service_action': {
      const action = String(args.action);
      if (scenario === 'Port conflict' && !action.startsWith('stop')) throw new Error('Port 8800 is already in use. Choose a different website port in Settings.');
      if (!data.runtimeReady) throw new Error('Runtime files are missing.');
      if (action.endsWith('_all')) {
        for (const id of ['nginx', 'mysql', 'mail'] as const) running(id, action === 'start_all');
      } else if (args.service === 'nginx' || args.service === 'mysql' || args.service === 'mail') {
        running(args.service, action !== 'stop');
      } else throw new Error('Unsupported simulated service action.');
      result = structuredClone(data); break;
    }
    case 'save_settings': {
      if (data.services.some(service => service.running)) throw new Error('Stop all services before changing settings.');
      const ports = args.ports as Ports;
      const values = Object.values(ports);
      if (values.some(port => !Number.isInteger(port) || port < 1024 || port > 65535) || new Set(values).size !== values.length) throw new Error('Choose distinct ports between 1024 and 65535.');
      const version = String(args.version);
      if (!data.phpVersions[version]) throw new Error('PHP version is unavailable.');
      data.ports = { ...ports }; data.phpVersion = version;
      for (const service of data.services) {
        service.port = ports[service.id === 'nginx' ? 'website' : service.id];
        service.message = null;
      }
      if (scenario === 'Port conflict' && ports.website !== 8800) scenario = 'Stopped';
      result = structuredClone(data); break;
    }
    case 'cli_integration': configureCli(Boolean(args.enabled)); result = structuredClone(data); break;
    case 'pick_folder': return joinPath(home(), 'Code', 'new-site', 'public') as T;
    case 'save_project': {
      const project = args.project as ProjectConfig;
      // Match the backend's deny_unknown_fields so the form cannot send display-only fields.
      if (Object.keys(project).sort().join() !== 'domain,https,indexFallback,name,root') throw new Error(`invalid args \`project\` for command \`save_project\`: ${Object.keys(project).join(', ')}`);
      const others = data.projects.filter(item => item.domain !== args.previous);
      if (!project.name.trim()) throw new Error('Enter a site name up to 80 characters.');
      if (!/^[a-z0-9]([a-z0-9-]{0,61}[a-z0-9])?$/.test(project.domain)) throw new Error(`${project.domain}.test is not available. Use lowercase letters, numbers, and hyphens.`);
      if (others.some(item => item.domain === project.domain)) throw new Error(`${project.domain}.test is already used by another site.`);
      const saved = { ...project, available: true, domainReady: true };
      data.projects = args.previous ? data.projects.map(item => item.domain === args.previous ? saved : item) : [...data.projects, saved];
      syncDomains(); result = structuredClone(data); break;
    }
    case 'remove_project':
      data.projects = data.projects.filter(item => item.domain !== args.domain);
      if (!data.projects.some(item => item.https)) data.httpsTrusted = false;
      result = structuredClone(data); break;
    case 'sync_domains': syncDomains(); result = structuredClone(data); break;
    case 'set_project_https': {
      const project = data.projects.find(item => item.domain === args.domain);
      if (!project) throw new Error('This site is no longer registered.');
      project.https = Boolean(args.enabled);
      if (project.https) data.httpsTrusted = true;
      else if (!data.projects.some(item => item.https)) data.httpsTrusted = false;
      result = structuredClone(data); break;
    }
    case 'remove_https_trust':
      if (data.projects.some(item => item.https)) throw new Error('Turn off HTTPS for every site before removing certificate trust.');
      data.httpsTrusted = false; result = structuredClone(data); break;
    case 'backup_database':
      if (!data.services.find(service => service.id === 'mysql')?.ready) throw new Error('Start MySQL before making a backup.');
      result = joinPath(data.dataDir, 'backups', 'student-databases-ui-test.sql'); break;
    case 'read_logs':
      if (scenario === 'Logs empty' || scenario === 'Logs unreadable') { result = { entries: [], unavailable: scenario === 'Logs unreadable' ? ['php.log: permission denied'] : [], limited: false }; break; }
      if (scenario === 'Logs under load') { result = { entries: Array.from({ length: 900 }, (_, index) => ({ id: `burst-${index}`, source: ['php', 'mysql', 'mail'][index % 3], file: 'test.log', severity: index === 0 ? 'error' : 'unknown', time: index === 0 ? new Date(1792324800000).toISOString() : null, text: `burst-log-${index} 日本 မြန်မာ` })), unavailable: [], limited: true }; break; }
      result = { entries: [
      { id: 'php-error', source: 'php', file: 'php.log', severity: 'error', time: '2026-10-18T12:00:00Z', text: 'PHP Fatal error: Uncaught Error: Call to undefined function missing_function() in lesson/index.php:4\nStack trace:\n#0 {main}\n  thrown in lesson/index.php on line 4' },
      { id: 'request-failed', host: 'lesson.test', source: 'requests', file: 'access.log', severity: 'error', time: '2026-10-18T12:00:01Z', text: 'GET http://lesson.test:8800 /index.php → 500 · 0.003 s' },
      { id: 'mysql-info', source: 'mysql', file: 'mysql.log', severity: 'info', time: '2026-10-18T12:00:02Z', text: '[System] Ready for local connections.' },
      { id: 'request-ok', host: 'shop.test', source: 'requests', file: 'access.log', severity: 'info', time: '2026-10-18T12:00:03Z', text: 'GET https://shop.test:8843 / → 200 · 0.002 s' },
      { id: 'mail-output', source: 'mail', file: 'mail-process.log', severity: 'unknown', time: null, text: '[UI TEST — simulated mail log] custom diagnostic <script>safe text</script>' },
    ], unavailable: [], limited: false }; break;
    case 'diagnostic_report': result = JSON.stringify({
      schema: 1, product: 'Fairway XMP', version: data.version, simulated: true,
      build: { commit: 'ui-test', modifiedSources: false, debug: true },
      system: { os: platform === 'Windows' ? 'windows' : 'macos', version: platform === 'Windows' ? 'Windows 11 (simulated)' : 'macOS 15 (simulated)', architecture: platform === 'Windows' ? 'x86_64' : 'aarch64' },
      runtimeReady: data.runtimeReady, runtimeVersions: data.runtimeVersions, selectedPhp: data.phpVersion, selectedPhpVersion: data.phpVersions[data.phpVersion], selectedPhpLegacy: ['8.0', '8.1'].includes(data.phpVersion),
      services: data.services.map(({ id, port, running, ready }) => ({ id, port, running, ready })),
      commandLine: { installed: data.pathInstalled, phpDetectedByApp: data.pathPhp === null ? 'not-detected' : data.pathPhp === data.cliPhp ? 'fairway' : 'other-installation', composerDetectedByApp: data.pathComposer === null ? 'not-detected' : data.pathComposer === data.cliComposer ? 'fairway' : 'other-installation' },
      excluded: ['credentials', 'database contents', 'project files', 'logs', 'filesystem paths'],
    }, null, 2); break;
    case 'check_update': result = {
      configured: scenario === 'Update available' || scenario === 'Update failure',
      version: scenario.startsWith('Update ') ? '0.2.0-preview-with-a-long-version-label' : null,
      notes: 'Simulated release for visual review. Includes PHP selection, service controls, and database backups.',
    }; break;
    case 'install_update': {
      emit('fairway-operation-started', 'Downloading update…');
      try {
        emit('fairway-update-progress', { phase: 'downloading', downloaded: 25, total: 100 });
        await new Promise(resolve => setTimeout(resolve, 600));
        if (scenario === 'Update failure') throw new Error('Could not download the update. Your services are still available. Your projects and database data have been kept. Try again later.');
        for (const phase of ['verifying', 'preparing', 'installing']) {
          emit('fairway-update-progress', { phase });
          await new Promise(resolve => setTimeout(resolve, 350));
        }
        throw new Error('UI test only: no update was downloaded or installed.');
      } finally { emit('fairway-operation-finished', null); }
    }
    case 'open_target': throw new Error(`UI test only: would open ${String(args.target)}. No external application was opened.`);
    default: throw new Error(`Unsupported UI test command: ${command}`);
  }
  emit('fairway-status', structuredClone(data));
  return result as T;
}
