import * as service from './api';
import * as explorer from './explorer';
import type { Port } from './workspace-model';
export const workspacePort: Port = {
  snapshot: service.managedPostgresSnapshot, health: service.checkManagedPostgresHealth,
  open: explorer.openPostgresConnection, close: explorer.closePostgresConnection,
  execute: explorer.executePostgres, cancel: explorer.cancelPostgres,
  service: (action, port) => {
    const confirmed = { confirmed: true as const };
    switch (action) {
      case 'provision': return service.provisionManagedPostgres(confirmed);
      case 'initialize': return service.initializeManagedPostgres({ ...confirmed, port: port! });
      case 'start': return service.startManagedPostgres(confirmed);
      case 'stop': return service.stopManagedPostgres(confirmed);
      case 'restart': return service.restartManagedPostgres(confirmed);
    }
  },
};
