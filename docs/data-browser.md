# Desktop data browser

The beta 2 candidate adds **Data browser** to the macOS and Windows desktop panels alongside phpMyAdmin. The existing Linux terminal edition is unchanged. Start MySQL from the browser's empty state or Environment. phpMyAdmin remains available and also needs Nginx running.

## Included

- Create/delete databases; rename an **empty** database, preserving its character set and collation. MySQL has no atomic database rename. Populated schemas are refused without changing their contents; use export/import into a new database and update project connection settings. Close other applications that might create objects during a rename.
- Create/rename/delete tables. The column form covers common integer, decimal, floating-point, character/text, Boolean, date/time, JSON, and BLOB types; length/precision/scale, unsigned, NULL, literal/NULL/current-timestamp defaults, primary keys, and integer auto-increment. Composite primary keys can be selected when creating a table. Engine, secondary indexes and foreign-key forms are deferred.
- Add/edit/delete ordinary columns and inspect the table's SQL. Existing primary keys, comments and text collations are preserved on supported column edits. Generated, fractional time, expression-default and other advanced definitions stay in SQL/phpMyAdmin rather than being simplified by a form.
- Browse rows, sort by a column, and page 50 rows at a time. Use the SQL editor for filtering. Insert using explicit values or SQL NULL; only auto-increment columns and columns with non-NULL defaults offer a default option. Nullable columns without a non-NULL default start at NULL; other columns start in value mode. Edit/delete requires a primary key or non-null unique key. Views are read-only; keyless tables allow browse/insert. Binary, BIT, and spatial values display as hex and are read-only in row forms.
- Drag the divider to widen the table list or data panel. The divider also supports Left/Right arrow keys (Shift for larger steps), Home/End, and double-click to reset. Hide/show tables from the panel toolbar to give wide results the full width; restoring the list retains its chosen width while the page stays open. Resizing the window keeps room for the data panel. Dropdowns use consistent control heights across desktop WebViews and browser previews.
- Insert row sits beside the table actions menu. Database and table export/rename/delete are grouped in their respective actions menus. Table metadata loads with the table, so switching Rows/Structure does not start another operation or dim the screen. Empty row lists have an inset message and the SQL heading stays on one line for ordinary names.
- Database/table changes keep the current workspace visible until the complete replacement is ready, then update the catalog, selection, rows, and structure together. Browsing temporarily blocks interaction without fading controls; one small loading indicator appears after 200 ms without shifting content. Database requests retain the native operation guard but do not show a second app-wide busy indicator. Sidebar navigation preserves the selected database/table, tab, SQL text/results/history, and table-list sizing, refreshing the catalog when returning. Leaving cancels an active request; returning waits for cancellation before starting a refresh.
- SQL editor with selection/all execution, Command/Ctrl+Enter, multiple result sets, affected-row counts, timing, cancellation, errors, and ten recent successful queries kept only while the page is mounted. SQL changes take effect immediately; Cancel cannot undo completed statements. The SQL heading’s question-mark tooltip holds execution/result-limit/import guidance. Each result grid scrolls internally and is capped at the smaller of 360 pixels or 45% of the window height.
- Export a whole database from the database controls, or the selected table from its heading, to the app's backups directory. Full SQL dump import is through the CLI below, including stored-program `DELIMITER` commands.

The native backend runs bundled PHP/PDO over private stdin/stdout; it adds no HTTP database endpoint. Credentials stay in the native backend. Form values use bound parameters and identifiers are validated/quoted. Numbers remain strings, including BIGINT/DECIMAL values that JavaScript cannot represent exactly. Row changes compare all original values, including case-sensitive text, to reject stale edits. A live primary/unique key is verified again before writing.

Database/table/column deletion and supported column changes require confirmation and first create a full SQL export of the selected database. A failed backup prevents the change. Row deletions use a confirmation dialog and do not export the database. Arbitrary SQL is explicitly an administrative tool and does not receive automatic backups. Completed statements in a failing SQL script may already have changed data; refresh before retrying. System databases are hidden/protected by forms; the SQL editor has administrator access.

The display caps query results at 200 rows and ten result sets. SELECT without an explicit LIMIT has a 201-row session cap. Queries use unbuffered results, a 128 MiB worker limit, an 8 MiB response limit, 15-second SELECT limit, five-second lock waits and a 30-second worker timeout. Cancel kills the current owned connection's query and closes its worker. Exports and protective backups finish before cancellation can be checked; full CLI imports/exports are not given the interactive query timeout.

## Desktop command line

Choose **Enable command-line tools** in Settings and open a new terminal. On macOS and Windows, the compact section lists PHP, Composer, and `fairway db`. Its question-mark help contains terminal checks, MySQL requirements, and upgrade guidance; command paths/detection are under Command details. Existing installations may need **Remove from user PATH** then **Enable command-line tools** once to create the new `fairway` launcher (`fairway.cmd` on Windows). Direct invocation also works:

```sh
"/Applications/Fairway XMP.app/Contents/MacOS/fairway" db --help
```

On Windows use Command Prompt or PowerShell; quote paths containing spaces. For SQL arguments, use double quotes in Command Prompt instead of the single quotes shown in the shell examples below.

```sh
fairway db list
fairway db tables school
fairway db create school
fairway db rename empty_school new_school --yes
fairway db drop school --yes
fairway db query school 'SELECT COUNT(*) FROM students'
fairway db export school --output ~/Desktop/school.sql
fairway db export school --table students --mode structure --output ~/Desktop/students-structure.sql
fairway db export school --mode data --output ~/Desktop/school-data.sql
fairway db import school ~/Desktop/school.sql --yes
```

MySQL must be running. Results are JSON. Export modes are `all`, `structure`, and `data`; without `--output`, exports go to the app's backups directory. An export is published only after success and never overwrites an existing file. Create the target database before importing. Single-database exports omit `CREATE DATABASE` so ordinary tables can be imported into another selected database. Views/routines/triggers may contain explicit original-schema references; review those when migrating names. Incoming SQL may explicitly switch databases. Import stops on SQL errors and may leave partial changes, with a pre-import backup available. Local client shell commands and LOCAL INFILE are disabled during import. Avoid other database writers or stopping MySQL during administration.

## Verification and review

`npm run dev:ui` provides the same interface with clearly labeled in-memory fixtures. Select Running, then Data browser. Fixtures cover ordinary/keyless tables, views, exact large numbers, NULL/empty text, CRUD/schema forms and query pending/error/cancel states. Fixture SQL intentionally supports only a few SELECT examples; it is not a SQL engine.

`npm run test:ui` runs Chromium and WebKit at normal/minimum window sizes and attaches screenshots to the HTML report. The native example exercises isolated bundled MySQL, actual schema/row SQL, keys, concurrency conflicts, Unicode, exact values, cancellation, SQL export/import and recovery guards:

```sh
cargo run --manifest-path src-tauri/Cargo.toml --example data_browser_smoke \
  --no-default-features --locked -- "$PWD/.runtime-work/data-browser-smoke"
```

Use a fresh scratch directory for each run. It starts its own MySQL on free ports and shuts it down afterward. Both desktop installer workflows also run this example. Browser fixtures do not establish native correctness, and neither replaces a human check in the native WebViews before release.

Both desktop candidate workflows run this real-database example and browser input through the native stdio test bridge. The latter starts isolated bundled MySQL and checks database/table creation, Unicode row insertion/editing, and exact SQL results. The bridge is test-only and opens no HTTP database endpoint. Native WebView and installer acceptance still require the checks in [beta acceptance](beta-acceptance.md).

## Windows filesystem paths

PHP startup configuration also uses compatible path aliases when available, so the database driver loads from user profiles containing Unicode characters. Project names and query/row text keep their original Unicode values.

MySQL Server on Windows restricts file paths to the current ANSI code page ([upstream restriction](https://dev.mysql.com/doc/refman/9.7/en/windows-restrictions.html)). Fairway uses existing NTFS short-name aliases for MySQL configuration, data and log paths when available, without moving or renaming student data. PHP, the terminal folder picker and project paths retain their Unicode paths. When aliases are unavailable and the paths cannot be represented in the active code page, MySQL startup reports the limitation and keeps existing data intact. A filesystem with short-name aliases or paths compatible with the Windows locale is required in that case.
