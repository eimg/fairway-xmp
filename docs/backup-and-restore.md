# Back up and restore student databases

Fairway's SQL backups contain student databases, tables, views, routines, events, and triggers. They exclude MySQL's system databases and account tables. Projects in `fairway/www` need their own file backup.

## Create and keep a backup

1. Start MySQL. In the desktop panel, choose **Back up databases**; on Linux, run `fairway backup`.
2. Wait for the completed-backup message and keep the resulting `.sql` file. A `.partial` file is unfinished and must not be used as a completed backup.
3. Copy important backups to another drive or location. The default backups directory lives beside your database data, so it is not protection against losing that disk.

Each export gets a unique name. A failed export is reported as an error and its partial file is removed; a previous backup is kept. Concurrent application operations are serialized.

## Restore a full backup

Use the same MySQL family that produced the backup, normally the current Fairway installation. Start MySQL and close applications that write to these databases. Before importing over existing databases, back up their current contents and copy that backup somewhere safe: the SQL dump can replace tables with matching names.

Use the bundled MySQL client with Fairway's private local client configuration. This supports dumps containing views, routines, triggers, and their original definers. The configuration keeps the administrator password out of command arguments; never share `admin.cnf` or the `credentials` file. Don't add `--force`: stop and investigate SQL errors rather than treating a partial import as complete.

### macOS

In Terminal, replace the backup path with your actual file:

```sh
"/Applications/Fairway XMP.app/Contents/Resources/runtime/mysql/bin/mysql" \
  --defaults-file="$HOME/Library/Application Support/dev.fairway.environment/config/admin.cnf" \
  --default-character-set=utf8mb4 < "/path/to/fairway-backup.sql"
```

### Windows

Open **Command Prompt**, not PowerShell, for the `<` file-redirection syntax below. Locate the installed application directory using its shortcut's **Open file location**. Replace `C:\path\to\Fairway XMP` and the backup filename:

```bat
"C:\path\to\Fairway XMP\runtime\mysql\bin\mysql.exe" --defaults-file="%LOCALAPPDATA%\dev.fairway.environment\config\admin.cnf" --default-character-set=utf8mb4 < "C:\Backups\fairway-backup.sql"
```

### Linux

For the default installation and data directories:

```sh
"$HOME/.local/share/fairway-xmp/current/runtime/mysql/bin/mysql" \
  --defaults-file="${XDG_DATA_HOME:-$HOME/.local/share}/dev.fairway.environment/config/admin.cnf" \
  --default-character-set=utf8mb4 < "/path/to/fairway-backup.sql"
```

For a custom installation prefix, replace the MySQL executable path accordingly. Keep the saved data directory and configured port in `admin.cnf`; don't substitute another installation's configuration.

## Verify recovery

Open phpMyAdmin and check database names, expected tables, row counts, and representative records. Open a real PHP project that reads the restored data. If the project uses views, triggers, routines, or events, verify those objects too. Keep the original backup until these checks pass.

The native smoke test imports its SQL dump through the bundled client into a **fresh isolated MySQL instance**. It verifies multiple databases, data, a view, foreign-key enforcement, a trigger, a procedure, a function, a disabled event, and a PHP response. It also checks repeated exports preserve previous files and failed export setup leaves no `.partial` files.

The physical `mysql-8.4-before-9.7-*` upgrade backup is different from a SQL export. Follow its included README, using the matching previous MySQL version and restoring credentials/settings/version markers together. Never open that physical backup with the newer server.
