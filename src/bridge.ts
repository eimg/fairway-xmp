import type { Snapshot } from './types';
import manifest from '../runtime-manifest.json';
import { version } from '../package.json';

declare global {
  interface Window {
    __TAURI_INTERNALS__?: {
      invoke<T>(command: string, args?: Record<string, unknown>): Promise<T>;
      transformCallback(callback: (event: { payload: unknown }) => void, once?: boolean): number;
      unregisterCallback(id: number): void;
    };
  }
}

export const isDesktop = !!window.__TAURI_INTERNALS__;
// Only the dedicated Vite development mode can load the simulated backend.
export const isUiTest = import.meta.env.DEV && import.meta.env.MODE === 'ui-test' && !isDesktop;
export const canControl = isDesktop || isUiTest;
const simulation = isUiTest ? import('./ui-test/backend') : null;

// Browser previews deliberately do not pretend to start real services.
export const preview: Snapshot = {
  ports: { website: 8800, phpmyadmin: 8801, mysql: 13306, php: 19000 },
  services: [
    { id: 'nginx', running: false, ready: false, port: 8800, pid: null, message: null },
    { id: 'php', running: false, ready: false, port: 19000, pid: null, message: null },
    { id: 'mysql', running: false, ready: false, port: 13306, pid: null, message: null },
    { id: 'phpmyadmin', running: false, ready: false, port: 8801, pid: null, message: null },
  ],
  documentRoot: '~/fairway/www', projects: [], domainsPending: false, httpsTrusted: false, os: '', dataDir: 'Fairway XMP application data',
  runtimeReady: false, runtimeVersions: {
    php: manifest.php, mysql: manifest.mysql, nginx: manifest.nginx,
    phpmyadmin: manifest.phpmyadmin, composer: manifest.composer,
  }, pathInstalled: false,
  phpVersion: manifest.php.split('.').slice(0, 2).join('.'), phpVersions: manifest.phpVersions,
  cliPhp: null, cliComposer: null, pathPhp: null, pathComposer: null, version,
};

export function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (simulation) return simulation.then(module => module.invoke<T>(command, args));
  if (!window.__TAURI_INTERNALS__) return Promise.reject(new Error('Open the Fairway XMP desktop app to use this control.'));
  return window.__TAURI_INTERNALS__.invoke<T>(command, args);
}

export async function listen(event: string, callback: (payload: unknown) => void): Promise<() => void> {
  if (simulation) return (await simulation).listen(event, callback);
  const api = window.__TAURI_INTERNALS__;
  if (!api) return () => {};
  const handler = api.transformCallback(e => callback(e.payload));
  const eventId = await api.invoke<number>('plugin:event|listen', { event, target: { kind: 'Any' }, handler });
  return () => {
    void api.invoke('plugin:event|unlisten', { event, eventId }).catch(() => {});
    api.unregisterCallback(handler);
  };
}
