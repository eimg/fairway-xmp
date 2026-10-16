//! Test-only stdio bridge: exercise the React terminal against the real Rust PTY
//! and packaged PHP/Composer, without exposing a server or touching user data.
use fairway_core::{
    config::{Layout, Ports},
    data_browser,
    manager::Manager,
    terminal::{self, Terminals},
};
use serde_json::{json, Value};
use std::{
    io::{self, BufRead, Write},
    net::TcpListener,
    path::PathBuf,
    sync::{atomic::AtomicBool, Arc, Mutex},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    let root = PathBuf::from(
        args.get(1)
            .ok_or("Pass an empty isolated test directory and runtime path.")?,
    );
    let runtime = PathBuf::from(args.get(2).ok_or("Pass the packaged runtime path.")?);
    if !root.is_absolute() || !runtime.is_absolute() || root.exists() {
        return Err(
            "Use a new absolute isolated test directory and an absolute runtime path.".into(),
        );
    }
    let layout = Layout::at(
        root.join("Student May's home 日本"),
        root.join("data 日本"),
        runtime,
    );
    let mut manager = Manager::new(layout).map_err(io::Error::other)?;
    let project = manager.layout.www.join("Project with spaces 日本");
    std::fs::create_dir_all(&project)?;
    std::fs::write(
        manager.layout.www.join(".fairway-terminal-location"),
        b"COURSEWORK_FOLDER_VERIFIED",
    )?;
    std::fs::write(
        project.join(".fairway-terminal-location"),
        b"PROJECT_FOLDER_VERIFIED",
    )?;
    let listeners: Vec<_> = (0..4)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)))
        .collect::<Result<_, _>>()?;
    let ports: Vec<_> = listeners
        .iter()
        .map(|listener| listener.local_addr().unwrap().port())
        .collect();
    manager
        .save_ports(Ports {
            website: ports[0],
            phpmyadmin: ports[1],
            mysql: ports[2],
            php: ports[3],
        })
        .map_err(io::Error::other)?;
    drop(listeners);
    let terminals = Arc::new(Terminals::default());
    let manager = Arc::new(Mutex::new(manager));
    let mut workers = Vec::new();
    for line in io::stdin().lock().lines() {
        let request: Value = serde_json::from_str(&line?)?;
        let manager = manager.clone();
        let terminals = terminals.clone();
        // Tauri dispatches PTY read/write/resize independently. A serial test
        // transport can deadlock ConPTY waiting for its cursor-query response.
        workers.push(std::thread::spawn(move || {
            respond(request, &manager, &terminals)
        }));
    }
    terminals.close_all();
    for worker in workers {
        worker
            .join()
            .map_err(|_| io::Error::other("Bridge worker failed"))??;
    }
    terminals.close_all();
    manager
        .lock()
        .unwrap()
        .stop_all()
        .map_err(io::Error::other)?;
    Ok(())
}

fn respond(request: Value, manager: &Mutex<Manager>, terminals: &Terminals) -> io::Result<()> {
    let args = &request["args"];
    let id = args["id"].as_str().unwrap_or_default();
    let result: Result<Value, String> = (|| {
        Ok(match request["command"].as_str().unwrap_or_default() {
            "service_action" if args["action"] == "start" && args["service"] == "mysql" => {
                let mut manager = manager.lock().unwrap();
                manager.start("mysql").map_err(|error| {
                    format!(
                        "{error} {} {}",
                        std::fs::read_to_string(manager.layout.data.join("logs/mysql.log"))
                            .unwrap_or_default(),
                        std::fs::read_to_string(manager.layout.data.join("logs/mysql-process.log"))
                            .unwrap_or_default(),
                    )
                })?;
                serde_json::to_value(manager.snapshot()).unwrap()
            }
            "data_browser" => {
                let layout = manager.lock().unwrap().layout.clone();
                data_browser::execute(
                    &layout,
                    args["request"].clone(),
                    Arc::new(AtomicBool::new(false)),
                )?
            }
            "snapshot" => serde_json::to_value(manager.lock().unwrap().snapshot()).unwrap(),
            "plugin:event|listen" => json!(1),
            "plugin:event|unlisten" => Value::Null,
            "terminal_context" => {
                serde_json::to_value(terminal::context(&manager.lock().unwrap().layout)?).unwrap()
            }
            "terminal_start" => serde_json::to_value(terminals.start(
                &manager.lock().unwrap().layout.clone(),
                serde_json::from_value(args["request"].clone()).map_err(|e| e.to_string())?,
            )?)
            .unwrap(),
            "terminal_read" => serde_json::to_value(terminals.read(id)?).unwrap(),
            "terminal_write" => {
                terminals.write(id, args["data"].as_str().unwrap_or_default())?;
                Value::Null
            }
            "terminal_resize" => {
                terminals.resize(
                    id,
                    args["cols"].as_u64().unwrap_or(80) as u16,
                    args["rows"].as_u64().unwrap_or(24) as u16,
                )?;
                Value::Null
            }
            "terminal_close" => {
                terminals.close(id);
                Value::Null
            }
            _ => return Err("Unsupported test bridge command.".into()),
        })
    })();
    let response = match result {
        Ok(value) => json!({"requestId": request["requestId"], "value": value}),
        Err(error) => json!({"requestId": request["requestId"], "error": error}),
    };
    let mut stdout = io::stdout().lock();
    writeln!(stdout, "{response}")?;
    stdout.flush()?;
    Ok(())
}
