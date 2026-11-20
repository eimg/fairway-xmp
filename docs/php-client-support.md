# PHP clients and worker support

Source builds include `sockets`, `soap` and `ldap` in every bundled PHP version (8.2–8.6) on Windows, macOS and Linux. CLI/Composer projects and websites use the same selected PHP version and generated configuration. macOS and Linux also include `pcntl` and `posix` for CLI workers. PHP does not provide these two extensions on Windows; Unix process/signal-dependent workers need a Unix environment.

These are client capabilities. Supply the external services your project needs and configure their connection details in the project. Fairway adds no RabbitMQ, LDAP or Redis server, service controls, Data browser integration or external-service backups.

## Check a project

Enable Fairway's command-line tools in Settings and open a new terminal. On Linux, use `fairway php` and `fairway composer` if PATH integration is disabled.

```sh
php --ri sockets
php --ri soap
php --ri ldap
composer check-platform-reqs
```

On macOS/Linux, also check `php --ri pcntl` and `php --ri posix`. Run Composer from the project directory after installing its dependencies. Its platform check reports the requirements of the project's installed packages, including any additional extensions. Use `php -v` and `php --ini` to confirm that commands use the intended Fairway installation. Downloaded builds predating this support need an update; these instructions do not change an installed runtime.

## Connect to external services

| Project need | What Fairway supplies | What the project supplies |
| --- | --- | --- |
| PostgreSQL | `pdo_pgsql`, `pgsql` and client libraries | Server and credentials; see [PostgreSQL clients](postgres-client-support.md) |
| Redis | PHP's stream/TLS facilities | `composer require predis/predis`, a server and connection settings |
| RabbitMQ using php-amqplib | `sockets` and `mbstring` | `composer require php-amqplib/php-amqplib`, a broker and connection settings |
| SOAP APIs | Native `SoapClient` and `SoapServer` | Endpoint, authentication and any WSDL required by the API |
| LDAP / Active Directory | Native LDAP API and client libraries | Directory endpoint, bind/search configuration, permissions and trusted certificates |
| Unix CLI workers | `pcntl` and `posix` | Framework worker command and its queue/service configuration |

For Laravel with Predis, set `REDIS_CLIENT=predis` and the actual Redis host, port, credentials and database in the project's configuration. Predis does not satisfy a dependency on `ext-redis` or provide PHP's native `Redis` class. Likewise, php-amqplib does not satisfy `ext-amqp`; integrations such as Symfony's standard AMQP Messenger transport require that separate native extension. See [Laravel Redis](https://laravel.com/framework/docs/13.x/redis), [php-amqplib requirements](https://github.com/php-amqplib/php-amqplib/blob/master/composer.json) and [Symfony AMQP requirements](https://github.com/symfony/amqp-messenger/blob/8.0/composer.json).

Project `.env` files are read by their frameworks; Fairway and PHP do not automatically load them. Configure TLS according to the service/provider. LDAP uses its client library's trust settings, separately from PHP's `openssl.cafile`; an organization's private CA may need explicit LDAP trust configuration. Directory authentication methods depend on the upstream client libraries and server configuration. No directory credentials are saved by Fairway.

PCNTL enables signal-based queue worker timeouts and process control on Unix. Run workers from the CLI rather than a web request. This support does not make Unix-only worker packages compatible with Windows. See [Laravel worker timeouts](https://laravel.com/framework/docs/13.x/queues#timeout) and [PHP PCNTL platform support](https://www.php.net/manual/en/pcntl.installation.php).

## Build and verify

The common extensions and platform-specific worker extensions are required by `runtime-manifest.json`. Windows enables the existing upstream extension DLLs. macOS uses the compiled extensions already present in the pinned PHP bottles. Linux enables the extensions during PHP compilation and installs `libldap2-dev` on the builder only; the existing dependency/source collection includes LDAP's native library closure and corresponding notices/sources. Target machines need no separately installed LDAP library. Linux build reuse checks both common and Linux-specific extension requirements.

After preparing the runtime, run:

```sh
python3 scripts/prepare_runtime.py --verify-only
cargo run --manifest-path src-tauri/Cargo.toml --example php_extension_smoke --no-default-features --locked -- /path/to/fresh-extension-smoke
```

The smoke example uses production-generated configuration for every bundled PHP version, through CLI and Nginx/PHP. It checks a binary/Unicode TCP socket round trip, SOAP serialization and dispatch, LDAP client initialization/options/filter escaping, and Unix CLI signal delivery, child processes and exit status. LDAP initialization is lazy: this check does not contact a directory or establish real LDAP authentication/TLS compatibility. The socket listener is disposable and loopback-only; no external service is required and MySQL remains stopped. The example cleans up its services; its isolated test directory remains available for inspection.

Installer workflows run the check on Windows, macOS and Linux. The signed macOS PHP check also exercises the fixture after signing.
