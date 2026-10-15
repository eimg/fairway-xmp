export type ServiceId = 'nginx' | 'php' | 'mysql' | 'phpmyadmin';
export type Ports = { website: number; phpmyadmin: number; mysql: number; php: number };
export type ServiceStatus = {
  id: ServiceId; running: boolean; ready: boolean; port: number;
  pid: number | null; message: string | null;
};
export type Project = { name: string; url: string; kind: 'php' | 'laravel' | 'incomplete' | 'invalid'; message: string | null; root: string };
export type Snapshot = {
  ports: Ports; services: ServiceStatus[]; documentRoot: string; projects: Project[]; domainsReady: boolean; dataDir: string;
  runtimeReady: boolean; runtimeVersions: Record<string, string> | null;
  phpVersion: string; phpVersions: Record<string, string>;
  pathInstalled: boolean; pathPhp: string | null; pathComposer: string | null; version: string;
};
export type UpdateInfo = { configured: boolean; version: string | null; notes: string | null };
