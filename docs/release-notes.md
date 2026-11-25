# Fairway XMP 0.2.0-rc.1 — personal fork candidate

Prepared release-note draft for `RizzRioo06/fairway-xmp`. No successful candidate
run, signed installer or publication is claimed yet. Add the final candidate run
and source commit after verification.

- Adds a macOS arm64 PostgreSQL workspace with database/schema/table/view browsing,
  pagination, sorting, filters, columns, indexes and view definitions.
- Adds an SQL editor with a read-only default, exact-statement modifying SQL
  authorization, cancellation, exact text/NULL results and uncertain-outcome messages.
- Supports explicit external loopback PostgreSQL 17.x / 18.6 sessions and explicitly
  managed 18.6 clusters with Keychain-only credentials.
- Includes the existing macOS Node Projects workspace, optional fnm/Node tools,
  PHP/Nginx/MySQL services, MySQL/SQLite browser, sites/HTTPS, mail, backups and
  terminal. Templates from the separate feature branch are not included.
- Configures fork release/update links and a separately signed PostgreSQL runtime
  workflow with strict manifest pinning before application compilation.

PostgreSQL execution requires macOS arm64. Remote/TLS connections, PostgreSQL
role/backup UI and existing-cluster adoption/upgrades are unavailable. Real external
evidence covers 17.12 on the development host. Signed runtime, signed-app Keychain,
installer and cross-platform candidate results remain unverified.

Normal updates preserve user data. PostgreSQL cluster migration and runtime
replacement are not automatic. Teacher installations will not automatically trust
the fork's updater key or follow its feeds.

See [fork release setup](fork-release.md) for remaining configuration and the
candidate/publication sequence.
