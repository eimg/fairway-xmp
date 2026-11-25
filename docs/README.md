# Documentation

Start with the [user guide](../README.md) or [development setup](../CONTRIBUTING.md). [PROJECT_SPEC.md](../PROJECT_SPEC.md) describes the current behavior and architecture.

| Guide | Use it for |
| --- | --- |
| [Build and release](build-and-release.md) | Packaging, signing, candidate CI, publication and update feeds |
| [Personal fork release](fork-release.md) | Features, fork credentials, PostgreSQL signing/pinning and remaining release work |
| [Free sharing builds](free-sharing.md) | No-paid-account Mac/Windows packages, manual installation/updates and platform limits |
| [Testing](testing.md) | Automated checks, isolated development profiles and optional device checks |
| [Maintenance](maintenance.md) | Updating runtime dependencies, current limitations and future improvements |
| [CI performance](ci-build-performance.md) | Cache reuse, build retries and artifact cleanup |
| [Desktop data browser](data-browser.md) | MySQL/SQLite operations, limits and desktop CLI commands |
| [Node tools](node-tools.md) | Native fnm, existing Node detection, activation, storage and removal |
| [Node Projects](node-projects.md) | macOS Node workspace, registry, runtime isolation, owned execution and bounded logs |
| [Node Projects Stage 5 verification](node-projects-stage5-verification.md) | Integration evidence, metadata fix, discovery timeout investigation and release limitations |
| [Node Projects manual testing](node-projects-manual-testing.md) | Isolated macOS launch, disposable fixtures, acceptance checks and issue reporting |
| [PostgreSQL integration](postgresql-integration.md) | Independent discovery/lifecycle, explorer and SQL backend, runtime/release limits |
| [PostgreSQL native workspace acceptance](postgresql-native-acceptance.md) | Real Summit access, prepared client runtime and isolated Mac GUI commands |
| [PostgreSQL clients](postgres-client-support.md) | Connecting PHP applications to a user-supplied server |
| [PHP settings](php-settings.md) | Per-version controls, persistent overrides and reset |
| [PHP clients and workers](php-client-support.md) | Sockets, SOAP, LDAP, Redis/Predis and Unix worker support |
| [Backup and restore](backup-and-restore.md) | Creating and restoring database backups |
| [Security boundaries](security-boundaries.md) | Process, HTTP, certificate, trust and renderer boundaries |

Development follows the current product requirements, without a prerelease feature freeze or a mandatory device-acceptance checklist. Build workflows verify artifacts; the owner decides when to publish. Older investigations and release records remain available in Git history and published GitHub releases. They are not current development instructions.
