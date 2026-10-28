# Documentation

For application setup, task instructions and troubleshooting, use the published
[English how-to guides](https://eimg.github.io/fairway-xmp/) or
[မြန်မာလမ်းညွှန်](https://eimg.github.io/fairway-xmp/mm/).

The guides below provide technical and contributor reference. Start with
[development setup](../CONTRIBUTING.md) for app contributions or the
[website source guide](../website/README.md) for documentation changes.
[PROJECT_SPEC.md](../PROJECT_SPEC.md) describes current behavior and architecture.

| Guide | Use it for |
| --- | --- |
| [Build and release](build-and-release.md) | Packaging, signing, candidate CI, publication and update feeds |
| [Testing](testing.md) | Automated checks, isolated development profiles and optional device checks |
| [Maintenance](maintenance.md) | Updating runtime dependencies, current limitations and future improvements |
| [CI performance](ci-build-performance.md) | Cache reuse, build retries and artifact cleanup |
| [Desktop data browser](data-browser.md) | MySQL/SQLite operations, limits and desktop CLI commands |
| [Node tools](node-tools.md) | Native fnm, existing Node detection, activation, storage and removal |
| [PostgreSQL clients](postgres-client-support.md) | Connecting PHP applications to a user-supplied server |
| [PHP settings](php-settings.md) | Per-version controls, persistent overrides and reset |
| [PHP clients and workers](php-client-support.md) | Sockets, SOAP, LDAP, Redis/Predis and Unix worker support |
| [Backup and restore](backup-and-restore.md) | Creating and restoring database backups |
| [Security boundaries](security-boundaries.md) | Process, HTTP, certificate, trust and renderer boundaries |

Development follows the current product requirements, without a prerelease feature freeze or a mandatory device-acceptance checklist. Build workflows verify artifacts; the owner decides when to publish. Older investigations and release records remain available in Git history and published GitHub releases. They are not current development instructions.
