import { invoke } from '../bridge';
import type { InitializeManagedPostgres, ManagedPostgresConfirmation, ManagedPostgresLogs, ManagedPostgresSnapshot } from './types';
import type { InspectPostgresRuntimeRequest, PostgresRuntimeCandidate, PostgresRuntimeInventory } from './types';

// No automatic invocation/startup polling. A later explicit UI can use these foundations.
export function discoverPostgresRuntimes(): Promise<PostgresRuntimeInventory> {
  return invoke('postgres_runtime_discover');
}
export function inspectPostgresRuntime(request: InspectPostgresRuntimeRequest): Promise<PostgresRuntimeCandidate> {
  return invoke('postgres_runtime_inspect', { request });
}

export function provisionManagedPostgres(request: ManagedPostgresConfirmation): Promise<ManagedPostgresSnapshot> {
  return invoke('postgres_managed_provision', { request });
}
export function initializeManagedPostgres(request: InitializeManagedPostgres): Promise<ManagedPostgresSnapshot> {
  return invoke('postgres_managed_initialize', { request });
}
export function startManagedPostgres(request: ManagedPostgresConfirmation): Promise<ManagedPostgresSnapshot> {
  return invoke('postgres_managed_start', { request });
}
export function stopManagedPostgres(request: ManagedPostgresConfirmation): Promise<ManagedPostgresSnapshot> {
  return invoke('postgres_managed_stop', { request });
}
export function restartManagedPostgres(request: ManagedPostgresConfirmation): Promise<ManagedPostgresSnapshot> {
  return invoke('postgres_managed_restart', { request });
}
export function managedPostgresSnapshot(): Promise<ManagedPostgresSnapshot> {
  return invoke('postgres_managed_snapshot');
}
export function checkManagedPostgresHealth(): Promise<ManagedPostgresSnapshot> {
  return invoke('postgres_managed_health');
}
export function managedPostgresLogs(): Promise<ManagedPostgresLogs> {
  return invoke('postgres_managed_logs');
}
