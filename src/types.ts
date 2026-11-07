export type ServiceId = 'nginx' | 'php' | 'mysql' | 'phpmyadmin' | 'mail';
export type StartupServiceId = 'nginx' | 'mysql' | 'mail';
export type StartupServices = Record<StartupServiceId, boolean>;
export type LoginItemStatus = { registered: boolean; state: 'notRegistered' | 'enabled' | 'requiresApproval' | 'notFound' | 'registered' | 'otherInstallation' };
export type Ports = { website: number; phpmyadmin: number; mysql: number; php: number; mail: number; smtp: number };
export type ServiceStatus = {
  id: ServiceId; running: boolean; ready: boolean; port: number;
  pid: number | null; message: string | null; messageGeneration?: number;
};
export type ProjectConfig = { name: string; domain: string; root: string; https: boolean; indexFallback: boolean };
export type Project = ProjectConfig & { available: boolean; domainReady: boolean };
export type Snapshot = {
  ports: Ports; services: ServiceStatus[]; documentRoot: string; projects: Project[]; sitesError: string | null; domainsPending: boolean; domainsApprovalPending?: boolean; httpsTrusted: boolean; httpsTrustUpdate?: boolean; os: string; dataDir: string;
  runtimeReady: boolean; runtimeVersions: Record<string, string> | null; mailAvailable: boolean;
  phpVersion: string; phpVersions: Record<string, string>;
  pathInstalled: boolean; cliPhp: string | null; cliComposer: string | null;
  pathPhp: string | null; pathComposer: string | null; version: string;
};
export type UpdateInfo = { configured: boolean; version: string | null; notes: string | null };
export type NodeBinary = { version: string; path: string };
export type NodeInstallation = NodeBinary & { source: string };
export type NodeInstallationProgress = { id: string; version: string; phase: 'starting' | 'installing' | 'activating' };
export type NodeStatus = {
  installation?: NodeInstallationProgress | null;
  externalFnm: string | null; readOnlyReason: string | null;
  fnmPath: string; fnmVersion: string | null; fnmError?: string | null; directory: string;
  detected: NodeBinary | null; detectionError: string | null; managed: NodeBinary | null;
  installations: NodeInstallation[];
  installed: string[]; defaultVersion: string | null; enabled: boolean;
};
export type StartupStatus = { blocking: boolean; errors: string[]; dataDir: string | null };
export type UpdateProgress = { phase: 'downloading' | 'verifying' | 'preparing' | 'installing'; downloaded?: number; total?: number | null };
export type MailAddress = { Name: string; Address: string };
export type MailSummary = { ID: string; Read: boolean; From: MailAddress | null; To: MailAddress[] | null; Subject: string; Created: string; Snippet: string; Attachments: number };
export type MailList = { messages: MailSummary[]; messages_count: number; messages_unread: number };
export type MailAttachment = { PartID: string; FileName: string; ContentType: string; Size: number };
export type MailMessage = { ID: string; From: MailAddress | null; To: MailAddress[] | null; Cc: MailAddress[] | null; Subject: string; Date: string; HTML: string; Text: string; Attachments: MailAttachment[] | null; InlineImages?: Record<string, string>; InlineImagesIncomplete?: boolean };
export type LogEntry = { host?: string | null; id: string; source: string; file: string; severity: string; time: string | null; text: string };
export type LogSnapshot = { revision?: string; entries: LogEntry[]; unavailable: string[]; limited: boolean };
export interface PhpSettings {
  memoryLimitMb: number;
  maxExecutionTime: number;
  uploadMaxFilesizeMb: number;
  timezone: string;
  displayErrors: boolean;
  sessionUploadProgress: boolean;
}

export interface PhpSettingsView {
  version: string;
  settings: PhpSettings;
  customized: boolean;
  timezones: string[];
}
