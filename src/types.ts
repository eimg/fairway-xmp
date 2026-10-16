export type ServiceId = 'nginx' | 'php' | 'mysql' | 'phpmyadmin';
export type Ports = { website: number; phpmyadmin: number; mysql: number; php: number };
export type ServiceStatus = {
  id: ServiceId; running: boolean; ready: boolean; port: number;
  pid: number | null; message: string | null;
};
export type ProjectConfig = { name: string; domain: string; root: string; https: boolean; indexFallback: boolean };
export type Project = ProjectConfig & { available: boolean; domainReady: boolean };
export type Snapshot = {
  ports: Ports; services: ServiceStatus[]; documentRoot: string; projects: Project[]; domainsPending: boolean; httpsTrusted: boolean; os: string; dataDir: string;
  runtimeReady: boolean; runtimeVersions: Record<string, string> | null;
  phpVersion: string; phpVersions: Record<string, string>;
  pathInstalled: boolean; cliPhp: string | null; cliComposer: string | null;
  pathPhp: string | null; pathComposer: string | null; version: string;
};
export type UpdateInfo = { configured: boolean; version: string | null; notes: string | null };
