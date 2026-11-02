# Fairway XMP command line

This guide describes the current source branch. Published 0.1.1 desktop downloads
only expose the earlier `fairway db` commands; the cross-platform management CLI
ships with the next build.

Enable **Settings → Command-line tools** and reopen your terminal/editor. The
desktop installer includes the management CLI. Disabling PATH integration keeps
absolute-path invocation available. Existing enabled launchers refresh on launch.

macOS:

```sh
"/Applications/Fairway XMP.app/Contents/MacOS/fairway" --help
```

Windows: invoke `cli\fairway-cli.exe` inside the installation directory, or use
the managed `fairway.cmd` launcher. The console executable preserves stdin,
stdout, stderr and exit status for PowerShell, Command Prompt and agent processes.
The desktop `fairway.exe` also accepts management commands.

Linux keeps its installed `bin/fairway` command and terminal dashboard. On desktop
platforms, launching the desktop executable without arguments opens the panel;
the Windows console executable without arguments shows help.

## Common operations

```sh
fairway capabilities --json
fairway status --json
fairway start all --wait --json
fairway stop nginx --json
fairway restart mysql --json
fairway logs php --json
fairway ports --json
fairway ports 8800 8801 13306 19000 8802 11025 --json
fairway php-version 8.4 --json
fairway diagnostics --json
fairway backup --json
fairway shutdown --json
```

Start commands already wait for the manager's readiness checks; `--wait` makes
that intention explicit. Nginx controls its PHP worker. PHP CLI and Composer work
without starting web/database services. Port and PHP-version changes require
stopped services. Backup requires running MySQL. Logs return bounded recent output.

`fairway shutdown` waits for the active operation, stops services gracefully and
exits the supervisor. An open desktop can reconnect to an idle supervisor; services
stay stopped. Desktop Quit also stops the supervisor and closes embedded terminals.
Hiding the desktop or exiting a CLI client leaves the supervisor/services running.

## Sites and PHP settings

```sh
fairway site list --json
fairway site add shop /absolute/project/public --name "My Shop" --index --json
fairway site edit shop /absolute/moved-project/public --name "My Shop" --index --json
fairway site https shop on --json
fairway site hosts --json
fairway site remove shop --json
fairway php-settings --json
fairway php-settings 8.4 --file /absolute/php-settings.json --json
fairway php-settings 8.4 --reset --json
```

Use quoted Windows paths in PowerShell/Command Prompt. `--index` enables the front
controller used by Laravel. Edit replaces the site's root/name/app type and keeps
its HTTPS selection. Remove preserves project files.

PHP settings use the same validated fields and defaults as Settings. A settings
file is an object such as `{"memoryLimitMb":128,"timezone":"Asia/Yangon"}`; omitted
fields use defaults. Active PHP settings require stopped Nginx/PHP. See
[PHP settings](php-settings.md) for fields and behavior.

Hosts-file and certificate-trust changes can require OS approval. Use
`--non-interactive` for agent actions that must never open those prompts:

```sh
fairway site add shop /absolute/project/public --index --non-interactive --json
```

If address setup needs approval, the site remains saved and the command returns
an `approval_required` error. Inspect `site list`/`status`, ask the student to
complete the described step, then retry `site hosts`. Disabling HTTPS or removing
a site can also succeed before unused trust cleanup needs approval. Report that
partial result; do not assume an error rolled back the saved site change.

## Agent contract

`capabilities --json` works without starting a supervisor and identifies the
platform, commands, and schema version. Management commands accept `--json`;
success writes one JSON value to stdout. Status retains its existing snapshot
shape; other commands return objects, arrays or JSON strings as appropriate.
Errors exit with status 1 and write:

```json
{"ok":false,"schemaVersion":1,"error":{"code":"operation_failed","message":"..."}}
```

The other current error code is `approval_required`. A competing operation returns
`operation_failed` with a busy explanation. Check current state before retrying
a timed-out mutation. Read-only requests wait briefly for the controller and then
report busy rather than freezing behind a long backup or approval.

PHP, Composer, MySQL, mysqldump and shell commands preserve the bundled tool's
output, arguments and exit code; their output is not wrapped in JSON. Database
administration commands produce JSON; see [database CLI](data-browser.md#desktop-command-line).
Destructive database actions retain their confirmation and protective-backup rules.

## Shared supervisor

One detached process per user-data directory owns the service manager. Desktop,
CLI and Linux TUI use the same operations. Unix uses a private socket and lifetime
file lock; long paths use a verified private directory beneath `/tmp`. Windows
uses a named pipe with a current-user-only ACL and rejects remote clients.
Requests and responses are bounded and check runtime installation identity.

Multi-step desktop/CLI actions reserve the controller over one connection. The
reservation remains active through database work and OS approval, and releases
when the action ends or the client disconnects. Updates reserve installation
separately, back up initialized databases, stop the supervisor and prevent new
clients from restarting it during replacement. A failed installation releases
that reservation so service controls can reconnect.

No HTTP API or MCP server is included. Node management remains available through
the bundled upstream `fnm` CLI. Desktop UI preferences, mail browsing, SQLite
browsing, interactive updater controls and uninstall dialogs retain their existing
interfaces; this CLI covers environment management plus the MySQL command surface.

## Verification

Rust controller tests cover shared settings, contention, disconnect recovery,
shutdown ordering, installation locking and installation-identity rejection.
`tests/shared_cli_smoke.py` exercises debug CLI/desktop entry points against real
isolated runtimes, including HTTP/PDO, backups/import/export and PHP settings.
Debug-only identity checks reject release binaries before any isolated operation.
The smoke never enables PATH integration or changes system hosts/certificate trust.
