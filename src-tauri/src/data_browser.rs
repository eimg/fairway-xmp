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

fn php(layout: &Layout) -> Result<Command> {
    // Keep the worker off the command line: Windows process argument limits
    // and quoting must not depend on the size or contents of this PHP source.
    let script = layout.data.join("config/data-browser-worker.php");
    if fs::read(&script).ok().as_deref() != Some(WORKER.as_bytes()) {
        crate::config::atomic_write(&script, WORKER.as_bytes())?;
    }
    let mut command = Command::new(layout.php_exe("php")?);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .env("PHP_INI_SCAN_DIR", "")
        .env("MIBS", "")
        .arg("-c")
        .arg(crate::config::windows_ansi_path(
            &layout.data.join("config/php.ini"),
        ))
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
        || value.len() > 64
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
    connection(layout)?;
    let op = request["op"]
        .as_str()
        .ok_or("Choose a database operation.")?;
    if [
        "drop_database",
        "drop_table",
        "drop_column",
        "change_column",
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
        return Err("Import SQL files with fairway db import. The query editor accepts SQL scripts up to 1 MiB.".into());
    }
    if op == "rename_database" && request["confirmed"].as_bool() != Some(true) {
        return Err("Confirm the database rename first.".into());
    }
    let input = serde_json::to_vec(
        &json!({"port":layout.ports()?.mysql,"password":layout.secret()?,"request":request}),
    )
    .map_err(|e| e.to_string())?;
    if input.len() > 2 * 1024 * 1024 {
        return Err("The request is too large. Import SQL files using the command line.".into());
    }
    let mut child = php(layout)?
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
    name(&json!(database))?;
    connection(layout)?;
    if database.starts_with('-') {
        return Err("Invalid database name.".into());
    }
    let file = File::open(path).map_err(|e| e.to_string())?;
    export(layout, database, None, None, "all")?;
    let output = mysql(layout, "mysql")
        .args([
            "--binary-mode",
            "--default-character-set=utf8mb4",
            "--local-infile=0",
        ])
        .arg(database)
        .stdin(Stdio::from(file))
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .output()
        .map_err(|e| e.to_string())?;
    if !output.status.success() {
        return Err(format!(
            "Import failed; earlier statements may have completed. A backup was saved first. {}",
            String::from_utf8_lossy(&output.stderr)
        ));
    }
    Ok(())
}

pub fn cli(layout: &Layout, args: &[String]) -> Result<()> {
    if args.is_empty() || args[0] == "--help" {
        println!("Fairway database tools\n\nfairway db list\nfairway db tables DATABASE\nfairway db create DATABASE\nfairway db rename DATABASE NEW_NAME --yes\nfairway db drop DATABASE --yes\nfairway db query DATABASE SQL\nfairway db export DATABASE [--table TABLE] [--mode all|structure|data] [--output FILE]\nfairway db import DATABASE FILE --yes\n\nResults are JSON. MySQL must already be running. Exports never overwrite files.\nImports back up the target database first; SQL files can change other schemas.\nDatabase rename currently supports empty databases only.");
        return Ok(());
    }
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
        println!(
            "{}",
            json!({"path":export(layout,&db,table.as_deref(),output.as_deref(),&mode)?})
        );
        return Ok(());
    }
    if args[0] == "import" {
        if args.len() != 4 || args[3] != "--yes" {
            return Err("Use: fairway db import DATABASE FILE --yes".into());
        }
        import(layout, &get(1)?, Path::new(&get(2)?))?;
        println!(
            "{}",
            json!({"message":"SQL imported. A backup was saved first."})
        );
        return Ok(());
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
    println!(
        "{}",
        serde_json::to_string_pretty(&execute(layout, request, Arc::new(AtomicBool::new(false)))?)
            .map_err(|e| e.to_string())?
    );
    Ok(())
}
