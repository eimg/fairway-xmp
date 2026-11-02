//! Local database operations shared by desktop IPC and desktop command-line tools.
use crate::{
    config::{Layout, Result},
    process,
};
use serde_json::{json, Value};
use std::{
    collections::HashMap,
    fs::{self, File, OpenOptions},
    io::{BufRead, BufReader, Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Mutex,
    },
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

const WORKER: &str = include_str!("data_browser.php");
const SQLITE_WORKER: &str = include_str!("data_browser_sqlite.php");
const MAX_RESPONSE: u64 = 8 * 1024 * 1024;
#[derive(Default)]
pub struct Jobs(Mutex<HashMap<String, Arc<AtomicBool>>>);
impl Jobs {
    pub fn register(&self, id: &str) -> Result<Arc<AtomicBool>> {
        if id.is_empty() || id.len() > 100 {
            return Err("Invalid database request ID.".into());
        }
        let mut jobs = self
            .0
            .lock()
            .map_err(|_| "Database operations are unavailable.")?;
        if jobs.contains_key(id) || !jobs.is_empty() {
            return Err("Another database operation is running. Wait or cancel it first.".into());
        }
        let cancel = Arc::new(AtomicBool::new(false));
        jobs.insert(id.into(), cancel.clone());
        Ok(cancel)
    }
    pub fn finish(&self, id: &str) {
        if let Ok(mut jobs) = self.0.lock() {
            jobs.remove(id);
        }
    }
    pub fn cancel(&self, id: &str) {
        if let Ok(jobs) = self.0.lock() {
            if let Some(cancel) = jobs.get(id) {
                cancel.store(true, Ordering::SeqCst);
            }
        }
    }
}

fn php(layout: &Layout, sqlite: bool) -> Result<Command> {
    // Keep the worker off the command line: Windows process argument limits
    // and quoting must not depend on the size or contents of this PHP source.
    let (filename, worker) = if sqlite {
        ("config/data-browser-sqlite.php", SQLITE_WORKER)
    } else {
        ("config/data-browser-worker.php", WORKER)
    };
    let script = layout.data.join(filename);
    if fs::read(&script).ok().as_deref() != Some(worker.as_bytes()) {
        crate::config::atomic_write(&script, worker.as_bytes())?;
    }
    let mut command = Command::new(layout.php_exe("php")?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
        // PHP reads -c before initializing UTF-8 path handling. A Unicode data
        // directory without NTFS short names silently leaves php.ini unloaded.
        // Workers need only PDO: use an isolated startup configuration and load
        // its driver relative to the selected runtime, even on such volumes.
        command.current_dir(layout.php_root()?).args([
            "-n",
            "-d",
            "extension_dir=ext",
            "-d",
            if sqlite {
                "extension=pdo_sqlite"
            } else {
                "extension=pdo_mysql"
            },
        ]);
    }
    #[cfg(not(windows))]
    command.arg("-c").arg(layout.data.join("config/php.ini"));
    command
        .env("PHP_INI_SCAN_DIR", "")
        .env("MIBS", "")
        .args([
            "-d",
            "display_errors=stderr",
            "-d",
            "memory_limit=128M",
            "-f",
        ])
        .arg(script);
    Ok(command)
}
fn name(value: &Value) -> Result<&str> {
    let value = value.as_str().ok_or("Choose a database name.")?;
    if value.is_empty()
        || value.chars().count() > 64
        || value.chars().any(char::is_control)
        || ["mysql", "sys", "information_schema", "performance_schema"]
            .contains(&value.to_ascii_lowercase().as_str())
    {
        return Err("Choose a student database name of 1–64 characters.".into());
    }
    Ok(value)
}
fn connection(layout: &Layout) -> Result<()> {
    if !process::connected(layout.ports()?.mysql) {
        return Err("Start MySQL from Environment before opening the data browser.".into());
    }
    Ok(())
}
pub fn execute(layout: &Layout, request: Value, cancel: Arc<AtomicBool>) -> Result<Value> {
    execute_with_progress(layout, request, cancel, |_| {})
}

pub fn execute_with_progress(
    layout: &Layout,
    request: Value,
    cancel: Arc<AtomicBool>,
    progress: impl Fn(&str),
) -> Result<Value> {
    match request["driver"].as_str() {
        Some("sqlite") => return execute_sqlite(layout, request, cancel),
        None | Some("mysql") => (),
        _ => return Err("Choose MySQL or SQLite.".into()),
    }
    connection(layout)?;
    let op = request["op"]
        .as_str()
        .ok_or("Choose a database operation.")?;
    if [
        "drop_database",
        "drop_table",
        "drop_column",
        "change_column",
        "drop_index",
    ]
    .contains(&op)
    {
        if request["confirmed"].as_bool() != Some(true) {
            return Err("Confirm this destructive operation first.".into());
        }
        let database = name(&request["database"])?;
        export(layout, database, None, None, "all")?;
        if cancel.load(Ordering::SeqCst) {
            return Err("Canceled before changing the database. The backup was kept.".into());
        }
    }
    if op == "export" {
        return export(
            layout,
            name(&request["database"])?,
            request["table"].as_str(),
            None,
            request["mode"].as_str().unwrap_or("all"),
        )
        .map(|path| json!({"message":format!("SQL export saved: {}",path.display()),"path":path}));
    }
    if op == "import" {
        if request["confirmed"].as_bool() != Some(true) {
            return Err("Confirm the SQL import first.".into());
        }
        let path = Path::new(request["sqlFile"].as_str().ok_or("Choose a SQL file.")?);
        if !path.is_absolute() {
            return Err("Choose a SQL file using its full path.".into());
        }
        let backup = import_sql(layout, name(&request["database"])?, path, cancel, &progress)?;
        return Ok(
            json!({"message":format!("SQL imported into {}. Backup saved: {}", name(&request["database"])?, backup.display()), "backupPath":backup}),
        );
    }
    if op == "rename_database" && request["confirmed"].as_bool() != Some(true) {
        return Err("Confirm the database rename first.".into());
    }
    worker(
        layout,
        json!({"port":layout.ports()?.mysql,"password":layout.secret()?,"request":request}),
        false,
        cancel,
    )
}

fn php_sqlite_path(path: &Path) -> PathBuf {
    #[cfg(windows)]
    {
        use std::path::{Component, Prefix};
        let mut components = path.components();
        if let Some(Component::Prefix(prefix)) = components.next() {
            let mut local = match prefix.kind() {
                Prefix::VerbatimDisk(drive) => PathBuf::from(format!("{}:\\", drive as char)),
                Prefix::VerbatimUNC(server, share) => PathBuf::from(format!(
                    r"\\{}\{}",
                    server.to_string_lossy(),
                    share.to_string_lossy()
                )),
                _ => return path.to_path_buf(),
            };
            for component in components {
                if !matches!(component, Component::RootDir) {
                    local.push(component.as_os_str());
                }
            }
            return local;
        }
    }
    path.to_path_buf()
}

fn execute_sqlite(layout: &Layout, mut request: Value, cancel: Arc<AtomicBool>) -> Result<Value> {
    let path = Path::new(
        request["path"]
            .as_str()
            .ok_or("Open a SQLite file first.")?,
    );
    if !path.is_absolute() || !path.is_file() {
        return Err("This SQLite file is missing or is not a file. Open it again from its current location.".into());
    }
    let path = fs::canonicalize(path).map_err(|e| format!("Cannot open this SQLite file: {e}"))?;
    let mut file = File::open(&path).map_err(|e| e.to_string())?;
    let mut header = [0; 16];
    let size = file.read(&mut header).map_err(|e| e.to_string())?;
    if size != 0 && (size != 16 || &header != b"SQLite format 3\0") {
        return Err("Choose an unencrypted SQLite 3 database or an empty file.".into());
    }
    // PHP expands the PDO path itself and treats a leading "\\" as the current
    // drive root, so the \\?\ prefix from canonicalize never reaches the file.
    let path = php_sqlite_path(&path);
    request["path"] = json!(path);
    let op = request["op"]
        .as_str()
        .ok_or("Choose a database operation.")?
        .to_owned();
    let destructive = ["drop_table", "drop_column", "drop_index"].contains(&op.as_str());
    if destructive && request["confirmed"].as_bool() != Some(true) {
        return Err("Confirm this destructive operation first.".into());
    }
    if destructive || op == "backup" {
        let folder = layout.data.join("backups");
        fs::create_dir_all(&folder).map_err(|e| e.to_string())?;
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_millis();
        let backup = folder.join(format!(
            "sqlite-{stamp}-{:016x}.sqlite",
            rand::random::<u64>()
        ));
        OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&backup)
            .map_err(|e| e.to_string())?;
        let result = worker(
            layout,
            json!({"request":{"op":"backup", "path":path}, "backupPath":backup}),
            true,
            cancel.clone(),
        );
        if result.is_err() {
            let _ = fs::remove_file(&backup);
        }
        result?;
        if op == "backup" {
            return Ok(
                json!({"message":format!("SQLite backup saved: {}", backup.display()),"path":backup}),
            );
        }
        if cancel.load(Ordering::SeqCst) {
            return Err(
                "Canceled before changing the database. The SQLite backup was kept.".into(),
            );
        }
    }
    worker(layout, json!({"request":request}), true, cancel)
}

struct ChangedValueFiles(Vec<PathBuf>);
impl Drop for ChangedValueFiles {
    fn drop(&mut self) {
        for path in &self.0 {
            let _ = fs::remove_file(path);
        }
    }
}

fn stage_changed_values(
    layout: &Layout,
    input: &mut Value,
    sqlite: bool,
) -> Result<ChangedValueFiles> {
    let mut files = ChangedValueFiles(Vec::new());
    let request = input
        .get_mut("request")
        .ok_or("Missing database request.")?;
    if request.get("valueFiles").is_some() {
        return Err("Changed-value paths are private to the database worker.".into());
    }
    if sqlite || !matches!(request["op"].as_str(), Some("insert" | "update")) {
        return Ok(files);
    }
    let Some(values) = request.get_mut("values").and_then(Value::as_object_mut) else {
        return Ok(files);
    };
    let total: usize = values
        .values()
        .filter_map(Value::as_str)
        .map(str::len)
        .sum();
    if total > 16 * 1024 * 1024 {
        return Err("Changed text exceeds the form's 16 MiB limit. Change fewer fields together or use SQL.".into());
    }
    let mut paths = serde_json::Map::new();
    for (name, value) in values.iter() {
        let Some(text) = value.as_str() else {
            continue;
        };
        if text.len() <= 64 * 1024 && total <= 128 * 1024 {
            continue;
        }
        let path = layout.data.join(format!(
            "tmp/database-value-{:032x}.tmp",
            rand::random::<u128>()
        ));
        let mut options = OpenOptions::new();
        options.write(true).create_new(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&path).map_err(|error| error.to_string())?;
        files.0.push(path.clone());
        file.write_all(text.as_bytes())
            .map_err(|error| error.to_string())?;
        paths.insert(name.clone(), json!(path));
    }
    for name in paths.keys() {
        values.remove(name);
    }
    if !paths.is_empty() {
        request["valueFiles"] = Value::Object(paths);
    }
    Ok(files)
}

fn worker(
    layout: &Layout,
    mut input: Value,
    sqlite: bool,
    cancel: Arc<AtomicBool>,
) -> Result<Value> {
    if cancel.load(Ordering::SeqCst) {
        return Err("Database request canceled.".into());
    }
    // Large edited values travel once over IPC and through private temporary
    // files to PHP. The bounded JSON request contains no original large cells.
    let _files = stage_changed_values(layout, &mut input, sqlite)?;
    let input = serde_json::to_vec(&input).map_err(|e| e.to_string())?;
    if input.len() > 2 * 1024 * 1024 {
        return Err("The request is too large. Import SQL files using the command line.".into());
    }
    let mut child = php(layout, sqlite)?
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| e.to_string())?;
    let mut stdin = child.stdin.take().ok_or("Cannot open database input.")?;
    let writer = thread::spawn(move || stdin.write_all(&input));
    let stdout = child.stdout.take().ok_or("Cannot read database output.")?;
    let output = thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout
            .take(MAX_RESPONSE + 1)
            .read_to_end(&mut bytes)
            .map(|_| bytes)
    });
    let stderr = child.stderr.take().ok_or("Cannot read database errors.")?;
    let connection_id = Arc::new(AtomicU64::new(0));
    let id = connection_id.clone();
    let errors = thread::spawn(move || {
        let mut errors = String::new();
        for line in BufReader::new(stderr)
            .lines()
            .map_while(std::result::Result::ok)
        {
            if let Some(number) = line.strip_prefix("FAIRWAY_CONNECTION_ID=") {
                id.store(number.parse().unwrap_or(0), Ordering::SeqCst);
            } else if errors.len() < 16384 {
                errors.push_str(&line);
                errors.push('\n');
            }
        }
        errors
    });
    let start = Instant::now();
    let mut stopped = false;
    loop {
        if child.try_wait().map_err(|e| e.to_string())?.is_some() {
            break;
        }
        if cancel.load(Ordering::SeqCst) || start.elapsed() > Duration::from_secs(30) {
            let id = connection_id.load(Ordering::SeqCst);
            if id > 0 {
                let mut kill = process::command(crate::config::windows_ansi_path(
                    &layout.exe("mysql", "mysql"),
                ));
                kill.arg(format!(
                    "--defaults-file={}",
                    crate::config::windows_ansi_path(&layout.data.join("config/admin.cnf"))
                        .display()
                ))
                .args(["--connect-timeout=2", "--execute"])
                .arg(format!("KILL QUERY {id}"));
                let _ = kill.output();
            }
            let _ = child.kill();
            let _ = child.wait();
            stopped = true;
            break;
        }
        thread::sleep(Duration::from_millis(25));
    }
    let _ = writer.join();
    let bytes = output
        .join()
        .map_err(|_| "Cannot read database result.")?
        .map_err(|e| e.to_string())?;
    let errors = errors.join().unwrap_or_default();
    if stopped {
        return Err("Database request canceled or timed out. Completed SQL statements are not undone; refresh to check the current data.".into());
    }
    if bytes.len() as u64 > MAX_RESPONSE {
        return Err("Result exceeds 8 MiB. Select fewer rows or columns.".into());
    }
    let response: Value = serde_json::from_slice(&bytes).map_err(|_| {
        if errors.is_empty() {
            "The database worker did not return a valid result.".into()
        } else {
            format!("Database worker failed: {}", errors.trim())
        }
    })?;
    if let Some(error) = response["error"].as_str() {
        return Err(error.to_string());
    }
    response
        .get("result")
        .cloned()
        .ok_or_else(|| "Missing database result.".into())
}

fn mysql(layout: &Layout, executable: &str) -> Command {
    let mut command = process::command(crate::config::windows_ansi_path(
        &layout.exe("mysql", executable),
    ));
    command.arg(format!(
        "--defaults-file={}",
        crate::config::windows_ansi_path(&layout.data.join("config/admin.cnf")).display()
    ));
    if executable == "mysql" {
        command.arg("--connect-timeout=3");
    }
    command
}
pub fn export(
    layout: &Layout,
    database: &str,
    table: Option<&str>,
    destination: Option<&Path>,
    mode: &str,
) -> Result<PathBuf> {
    name(&json!(database))?;
    connection(layout)?;
    if !["all", "structure", "data"].contains(&mode) {
        return Err("Export mode must be all, structure, or data.".into());
    }
    if let Some(table) = table {
        if table.is_empty() || table.starts_with('-') || table.chars().any(char::is_control) {
            return Err("Invalid table name.".into());
        }
    }
    if database.starts_with('-') {
        return Err("Database names beginning with '-' cannot be exported.".into());
    }
    if !crate::config::mysql_cli_identifier_supported(database)
        || table.is_some_and(|name| !crate::config::mysql_cli_identifier_supported(name))
    {
        return Err("The Windows MySQL client cannot export non-ASCII database or table names with the current system code page. Use ASCII identifiers before retrying. Your data has been kept.".into());
    }
    fs::create_dir_all(layout.data.join("backups")).map_err(|e| e.to_string())?;
    let path = destination.map(Path::to_path_buf).unwrap_or_else(|| {
        layout.data.join("backups").join(format!(
            "database-{}-{:016x}.sql",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap_or_default()
                .as_nanos(),
            rand::random::<u64>()
        ))
    });
    let mut options = OpenOptions::new();
    options.create_new(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    if path.exists() {
        return Err("The export destination already exists. Choose a new filename.".into());
    }
    let partial = path.with_file_name(format!(
        ".fairway-export-{:016x}.partial",
        rand::random::<u64>()
    ));
    let file = options
        .open(&partial)
        .map_err(|e| format!("Cannot create export (existing files are kept): {e}"))?;
    let mut command = mysql(layout, "mysqldump");
    command.args([
        "--single-transaction",
        "--routines",
        "--triggers",
        "--events",
        "--hex-blob",
        "--no-tablespaces",
        "--set-gtid-purged=OFF",
    ]);
    if mode == "structure" {
        command.arg("--no-data");
    }
    if mode == "data" {
        command.args([
            "--no-create-info",
            "--skip-triggers",
            "--skip-routines",
            "--skip-events",
        ]);
    }
    command.arg(database);
    if let Some(table) = table {
        command.arg(table);
    }
    let result = (|| {
        let output = command
            .stdout(Stdio::from(file.try_clone().map_err(|e| e.to_string())?))
            .stderr(Stdio::piped())
            .output()
            .map_err(|e| e.to_string())?;
        if !output.status.success() {
            return Err(String::from_utf8_lossy(&output.stderr).into_owned());
        }
        file.sync_all().map_err(|e| e.to_string())?;
        // A same-directory hard link publishes the completed dump atomically and
        // cannot overwrite a file created after the initial destination check.
        fs::hard_link(&partial, &path)
            .map_err(|e| format!("Cannot publish export (existing files are kept): {e}"))?;
        Ok(path)
    })();
    drop(file);
    let _ = fs::remove_file(&partial);
    result
}
pub fn import(layout: &Layout, database: &str, path: &Path) -> Result<()> {
    import_sql(
        layout,
        database,
        path,
        Arc::new(AtomicBool::new(false)),
        &|_| {},
    )
    .map(|_| ())
}

fn import_sql(
    layout: &Layout,
    database: &str,
    path: &Path,
    cancel: Arc<AtomicBool>,
    progress: &dyn Fn(&str),
) -> Result<PathBuf> {
    name(&json!(database))?;
    connection(layout)?;
    if database.starts_with('-') {
        return Err("Invalid database name.".into());
    }
    if cancel.load(Ordering::SeqCst) {
        return Err("Import canceled before changing the database.".into());
    }
    if !path.is_file() {
        return Err("Choose an existing SQL file.".into());
    }
    let file = File::open(path).map_err(|e| format!("Cannot open the SQL file: {e}"))?;
    let metadata = file.metadata().map_err(|e| e.to_string())?;
    if !metadata.is_file() || metadata.len() == 0 {
        return Err("Choose a non-empty SQL file.".into());
    }
    progress("backup");
    let backup = export(layout, database, None, None, "all").map_err(|e| {
        format!("Could not create the pre-import backup. Nothing was imported. {e}")
    })?;
    if cancel.load(Ordering::SeqCst) {
        return Err(format!(
            "Import canceled before changing the database. Backup saved: {}",
            backup.display()
        ));
    }
    progress("import");
    let mut child = mysql(layout, "mysql")
        .args([
            "--binary-mode",
            "--default-character-set=utf8mb4",
            "--local-infile=0",
        ])
        .arg(database)
        .stdin(Stdio::from(file))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| {
            format!(
                "Cannot start SQL import: {e}. Backup saved: {}",
                backup.display()
            )
        })?;
    let mut stderr = child.stderr.take().expect("import stderr is piped");
    // Drain diagnostics without keeping an unbounded SQL error response in memory.
    let errors = thread::spawn(move || {
        let mut kept = Vec::new();
        let mut buffer = [0; 4096];
        while let Ok(count) = stderr.read(&mut buffer) {
            if count == 0 {
                break;
            }
            let take = count.min(16384_usize.saturating_sub(kept.len()));
            kept.extend_from_slice(&buffer[..take]);
        }
        String::from_utf8_lossy(&kept).into_owned()
    });
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Err(error) => break Err(format!("Cannot read import status: {error}")),
            Ok(None) => (),
        }
        if cancel.load(Ordering::SeqCst) {
            break Err("Import canceled".to_string());
        }
        thread::sleep(Duration::from_millis(50));
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let errors = errors.join().unwrap_or_default();
    let status = result.map_err(|e| {
        format!(
            "{e}; completed statements are not undone. Backup saved: {}",
            backup.display()
        )
    })?;
    if !status.success() {
        return Err(format!(
            "Import failed; earlier statements may have completed. Backup saved: {}\n{}",
            backup.display(),
            errors.trim()
        ));
    }
    Ok(backup)
}

pub fn cli(layout: &Layout, args: &[String]) -> Result<()> {
    if args.is_empty() || args[0] == "--help" {
        println!("Fairway database tools\n\nfairway db list\nfairway db tables DATABASE\nfairway db create DATABASE\nfairway db rename DATABASE NEW_NAME --yes\nfairway db drop DATABASE --yes\nfairway db query DATABASE SQL\nfairway db export DATABASE [--table TABLE] [--mode all|structure|data] [--output FILE]\nfairway db import DATABASE FILE --yes\n\nResults are JSON. MySQL must already be running. Exports never overwrite files.\nImports back up the target database first; SQL files can change other schemas.\nDatabase rename currently supports empty databases only.");
        return Ok(());
    }
    println!(
        "{}",
        serde_json::to_string_pretty(&cli_value(layout, args)?).map_err(|e| e.to_string())?
    );
    Ok(())
}

pub(crate) fn cli_value(layout: &Layout, args: &[String]) -> Result<Value> {
    let get = |n| {
        args.get(n)
            .cloned()
            .ok_or("Missing argument. Run fairway db --help.".to_string())
    };
    if args[0] == "export" {
        let db = get(1)?;
        let mut table = None;
        let mut output = None;
        let mut mode = "all".to_string();
        let mut i = 2;
        while i < args.len() {
            let value = get(i + 1)?;
            match args[i].as_str() {
                "--table" => table = Some(value),
                "--output" => output = Some(PathBuf::from(value)),
                "--mode" => mode = value,
                _ => return Err("Unknown export option.".into()),
            }
            i += 2;
        }
        return Ok(json!({"path":export(layout,&db,table.as_deref(),output.as_deref(),&mode)?}));
    }
    if args[0] == "import" {
        if args.len() != 4 || args[3] != "--yes" {
            return Err("Use: fairway db import DATABASE FILE --yes".into());
        }
        import(layout, &get(1)?, Path::new(&get(2)?))?;
        return Ok(json!({"message":"SQL imported. A backup was saved first."}));
    }
    let request = match args[0].as_str() {
        "list" if args.len() == 1 => json!({"op":"databases"}),
        "tables" if args.len() == 2 => json!({"op":"tables","database":get(1)?}),
        "create" if args.len() == 2 => json!({"op":"create_database","database":get(1)?}),
        "rename" if args.len() == 4 && args[3] == "--yes" => {
            json!({"op":"rename_database","database":get(1)?,"name":get(2)?,"confirmed":true})
        }
        "drop" if args.len() == 3 && args[2] == "--yes" => {
            json!({"op":"drop_database","database":get(1)?,"confirmed":true})
        }
        "query" if args.len() == 3 => json!({"op":"query","database":get(1)?,"sql":get(2)?}),
        _ => return Err("Unknown command or missing confirmation. Run fairway db --help.".into()),
    };
    execute(layout, request, Arc::new(AtomicBool::new(false)))
}

#[cfg(all(test, windows))]
mod tests {
    use super::php_sqlite_path;

    #[test]
    fn sqlite_worker_path_drops_verbatim_prefix() {
        let file =
            std::env::temp_dir().join(format!("fairway-sqlite-path-{}.sqlite", std::process::id()));
        std::fs::write(&file, b"").unwrap();
        let canonical = std::fs::canonicalize(&file).unwrap();
        let worker = php_sqlite_path(&canonical);
        assert!(canonical.to_string_lossy().starts_with(r"\\?\"));
        assert!(!worker.to_string_lossy().starts_with(r"\\?\"));
        assert!(worker.is_file());
        assert_eq!(std::fs::canonicalize(&worker).unwrap(), canonical);
        std::fs::remove_file(&file).unwrap();
    }
}

#[cfg(test)]
mod row_protocol_tests {
    use super::*;
    fn fixture() -> Layout {
        let root = std::env::temp_dir().join(format!(
            "fairway-value-test-{:032x}",
            rand::random::<u128>()
        ));
        fs::create_dir_all(root.join("data/tmp")).unwrap();
        Layout::at(root.join("home"), root.join("data"), root.join("runtime"))
    }
    #[test]
    fn identifiers_count_characters_and_reject_controls() {
        let unicode = "日本".repeat(32);
        assert_eq!(name(&json!(unicode)).unwrap(), unicode);
        assert!(name(&json!(format!("{unicode}a"))).is_err());
        assert!(name(&json!("line\nbreak")).is_err());
    }
    #[test]
    fn changed_values_are_private_bounded_and_removed_by_guard() {
        let layout = fixture();
        let text = "🟢é\n\\'".repeat(20000);
        let mut input = json!({"request":{"op":"update","values":{"body":text,"note":null}}});
        let files = stage_changed_values(&layout, &mut input, false).unwrap();
        assert!(input["request"]["values"].get("body").is_none());
        assert!(input["request"]["values"]["note"].is_null());
        assert!(serde_json::to_vec(&input).unwrap().len() < 1000);
        assert_eq!(fs::read_to_string(&files.0[0]).unwrap(), text);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&files.0[0]).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        let path = files.0[0].clone();
        drop(files);
        assert!(!path.exists());
        assert!(stage_changed_values(
            &layout,
            &mut json!({"request":{"op":"update","valueFiles":{"body":"/etc/passwd"}}}),
            false
        )
        .is_err());
        assert!(stage_changed_values(
            &layout,
            &mut json!({"request":{"op":"insert","values":{"a":"x".repeat(16*1024*1024),"b":"x"}}}),
            false
        )
        .is_err());
        assert_eq!(fs::read_dir(layout.data.join("tmp")).unwrap().count(), 0);
        fs::remove_dir_all(layout.data.parent().unwrap()).unwrap();
    }
    #[test]
    fn aggregate_values_stage_even_when_individual_fields_are_small() {
        let layout = fixture();
        let values: serde_json::Map<String, Value> = (0..100)
            .map(|i| (format!("column{i}"), json!("x".repeat(2000))))
            .collect();
        let mut input = json!({"request":{"op":"insert","values":values}});
        let files = stage_changed_values(&layout, &mut input, false).unwrap();
        assert_eq!(files.0.len(), 100);
        assert_eq!(input["request"]["values"].as_object().unwrap().len(), 0);
        drop(files);
        // SQLite retains its original protocol and limits.
        let mut input = json!({"request":{"op":"insert","values":{"body":"x".repeat(100000)}}});
        assert!(stage_changed_values(&layout, &mut input, true)
            .unwrap()
            .0
            .is_empty());
        assert!(input["request"].get("valueFiles").is_none());
        fs::remove_dir_all(layout.data.parent().unwrap()).unwrap();
    }
}
