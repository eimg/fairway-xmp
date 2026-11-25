# PostgreSQL client support

Current source builds include PHP's `pdo_pgsql` and `pgsql` extensions, with their client libraries, in every bundled PHP version (8.2–8.5) on Windows, macOS and Linux. Both PHP CLI/Composer and web requests through Nginx/PHP can use them. Verify driver availability when using a downloaded build.

Supply your own running PostgreSQL server, locally, in a container, or on another host. Configure your project with that server's host, port, database, username, password and any required TLS settings. Its network access and database permissions determine which operations your application can perform. PostgreSQL does not require Fairway's MySQL service to be running.

Fairway includes no PostgreSQL server, `psql`, `pg_dump` or PostgreSQL service controls. The desktop data browser supports MySQL/SQLite. Install any administration tools separately. Fairway's environment backups and updater protect its own MySQL data; external PostgreSQL databases keep their own backup and upgrade process. phpMyAdmin remains MySQL-only.

Phase 2 [Stage 1 architecture and discovery](postgresql-integration.md) adds only
read-only tool discovery and explicit version-inspection IPC. It does not change
this client-only packaging, start servers, connect to databases, or add a PostgreSQL
explorer yet.

## Verify the PHP drivers

Enable Fairway's command-line tools in Settings and open a new terminal. On Linux, use `fairway php` instead of `php` if PATH integration is disabled.

```sh
php --ri pdo_pgsql
php --ri pgsql
php -r 'var_export(PDO::getAvailableDrivers());'
```

The PDO driver list should include `pgsql`. If it does not, check that `php --ini` and `php -v` refer to the intended Fairway installation/version. Upgrade to a build containing this feature; an existing installation does not gain extensions from documentation changes.

## Connect from a project

For a local server on TCP port 5432 with an existing `learning` database, set `PGUSER` and `PGPASSWORD` in the environment used to run your project, or load the equivalent values through your framework's configuration. A framework `.env` file is read by the framework; Fairway and PHP do not automatically load it. Keep credentials out of committed source files.

```php
<?php
$db = new PDO(
    'pgsql:host=127.0.0.1;port=5432;dbname=learning;sslmode=disable;connect_timeout=5',
    (string) getenv('PGUSER'),
    (string) getenv('PGPASSWORD'),
    [PDO::ATTR_ERRMODE => PDO::ERRMODE_EXCEPTION]
);
echo $db->query('SELECT current_database()')->fetchColumn();
```

This example explicitly uses an unencrypted loopback connection. For a remote server, use the provider's host and TLS configuration. `sslmode=verify-full` verifies both the certificate chain and hostname; configure the trusted CA through libpq's `sslrootcert` connection parameter or `PGSSLROOTCERT`. PHP's `openssl.cafile` setting does not configure libpq's trust roots. See [PDO PostgreSQL connection parameters](https://www.php.net/manual/en/ref.pdo-pgsql.connection.php) and [PostgreSQL TLS settings](https://www.postgresql.org/docs/current/libpq-ssl.html).

Projects using PHP's native `pg_connect`/`pg_query_params` API are supported too. Connection details belong in the project, independently of Fairway's MySQL port and teaching account. Frameworks should select the `pgsql` driver and use the actual external server credentials.

## Build and verify

Runtime preparation requires both extensions in all bundled PHP versions. Mac and Windows reuse the client libraries supplied by their pinned PHP inputs. Linux builder prerequisites include `libpq-dev`; users need no system libpq package because the runtime bundles its dependency closure. Corresponding sources/notices cover the client libraries. Packaging currently includes client support only; runtime verification does not prohibit future PostgreSQL components.

After preparing the runtime, run:

```sh
python3 scripts/check_postgres_clients.py
# If the test server binaries are not discovered automatically:
python3 scripts/check_postgres_clients.py --postgres-bin /path/to/postgresql/bin
```

The check requires an external PostgreSQL installation and Cargo. It initializes a disposable cluster on an available loopback port with SCRAM password authentication, runs the Rust smoke example, then stops and removes only that test cluster. It does not start/stop an existing PostgreSQL service or put server files in the runtime. Every bundled PHP version is exercised through production-generated PHP configuration, both as CLI and through Nginx/PHP, while Fairway's MySQL remains stopped. Checks cover both APIs, rejected incorrect passwords, parameterized queries, Unicode, exact BIGINT/NUMERIC values, JSONB, writes and transaction rollback.

Candidate workflows run this check on macOS, Windows and Linux. macOS installs a server on the builder only, Windows uses the runner's `PGBIN`, and Linux installs a server on the builder only. No PostgreSQL server is packaged.

On Windows, the check grants the current user's SID inheritable access to its private test directory before creating the password file or cluster. PostgreSQL's `initdb` drops administrator group privileges, so a directory accessible only through those groups would otherwise reject the password file. This permission change applies only to the disposable test directory.
