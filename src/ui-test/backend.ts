import { preview } from '../bridge';
import { terminalRequest } from './terminal';
import type { Ports, ServiceId, Snapshot } from '../types';
import { cancelData, databaseRequest, resetData } from './data-browser';

// Fixture modules own event subscriptions and state. Reload them together so
// Fast Refresh cannot keep a screen subscribed to an obsolete simulation.
if (import.meta.hot) import.meta.hot.accept(() => window.location.reload());

export const scenarios = ['Stopped', 'Domains not set up', 'One domain set up', 'Removed project domain', 'Running', 'Partially running', 'Port conflict', 'Missing runtime', 'Update available', 'Update failure', 'CLI not detected by app', 'Other PHP on PATH', 'CLI disabled'] as const;
export type Scenario = typeof scenarios[number];
export type Platform = 'Windows' | 'macOS';
const events = new EventTarget();
let scenario: Scenario = 'Stopped';
export const defaultPlatform: Platform = navigator.platform.startsWith('Mac') ? 'macOS' : 'Windows';
let platform: Platform = defaultPlatform;
let data: Snapshot;

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
  scenario = next; platform = os;
  data = { ...structuredClone(preview), ...paths(), runtimeReady: next !== 'Missing runtime', pathInstalled: true };
  data.projects = [
    { name: 'hello', url: 'http://hello.test:8800/', httpsUrl: null, sslEnabled: false, domainEnabled: next !== 'Domains not set up', kind: 'php', message: null, root: joinPath(data.documentRoot, 'hello') },
    { name: 'laravel-app', url: 'http://laravel-app.test:8800/', httpsUrl: 'https://laravel-app.test:8443/', sslEnabled: true, domainEnabled: next !== 'Domains not set up' && next !== 'One domain set up', kind: 'laravel', message: null, root: joinPath(data.documentRoot, 'laravel-app', 'public') },
  ];
  if (next === 'Domains not set up' || next === 'One domain set up') {
    data.projects[1].sslEnabled = false;
    data.projects[1].httpsUrl = null;
  }
  if (next === 'Removed project domain') data.staleDomains = ['old-project'];
  configureCli(next !== 'CLI disabled');
  if (next === 'Running' || next === 'Partially running') running('nginx', true);
  if (next === 'Running') running('mysql', true);
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
  if (command.startsWith('terminal_')) return terminalRequest(command, args, data.documentRoot, data.phpVersions[data.phpVersion], data.runtimeReady) as T;
  if (command === 'snapshot') return structuredClone(data) as T;
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
        running('nginx', action === 'start_all'); running('mysql', action === 'start_all');
      } else if (args.service === 'nginx' || args.service === 'mysql') {
        running(args.service, action !== 'stop');
      } else throw new Error('Unsupported simulated service action.');
      result = structuredClone(data); break;
    }
    case 'save_settings': {
      if (data.services.some(service => service.running)) throw new Error('Stop all services before changing settings.');
      const ports = args.ports as Ports;
      const values = Object.values(ports);
      if (values.some(port => !Number.isInteger(port) || port < 1024 || port > 65535) || new Set(values).size !== 4) throw new Error('Choose distinct ports between 1024 and 65535.');
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
    case 'refresh_projects': result = structuredClone(data); break;
    case 'set_project_domain': {
      const project = data.projects.find(project => project.name === args.name);
      if (!project && args.enabled) throw new Error('Project not found.');
      if (!project && !data.staleDomains.includes(String(args.name))) throw new Error('Domain not found.');
      if (!project) data.staleDomains = data.staleDomains.filter(name => name !== args.name);
      else {
      if (!args.enabled && project.sslEnabled) throw new Error('Turn off HTTPS for this project first.');
      project.domainEnabled = Boolean(args.enabled);
      }
      result = structuredClone(data); break;
    }
    case 'set_project_ssl': {
      const project = data.projects.find(project => project.name === args.name);
      if (!project) throw new Error('Project not found.');
      if (args.enabled && !project.domainEnabled) throw new Error('Enable this project’s .test address first.');
      project.sslEnabled = Boolean(args.enabled);
      project.httpsUrl = project.sslEnabled ? `https://${project.name}.test:8443/` : null;
      result = structuredClone(data); break;
    }
    case 'backup_database':
      if (!data.services.find(service => service.id === 'mysql')?.ready) throw new Error('Start MySQL before making a backup.');
      result = joinPath(data.dataDir, 'backups', 'student-databases-ui-test.sql'); break;
    case 'read_log': result = `[UI TEST — simulated ${String(args.service)} log]\nService configuration loaded.\nReady for local connections.\nNo real services are running.`; break;
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
    case 'install_update': throw new Error(scenario === 'Update failure' ? 'Could not download the update. Your projects and database data have been kept. Try again later.' : 'UI test only: no update was downloaded or installed.');
    case 'open_target': throw new Error(`UI test only: would open ${String(args.target)}. No external application was opened.`);
    default: throw new Error(`Unsupported UI test command: ${command}`);
  }
  emit('fairway-status', structuredClone(data));
  return result as T;
}
