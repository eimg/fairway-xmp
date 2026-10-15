//! CI-only probe of real installed user data across installer replacement/removal.
use fairway_core::{
    config::{Layout, Ports, Result},
    diagnostics,
    manager::Manager,
    process,
};
use std::{fs, net::TcpListener, path::PathBuf};

fn run() -> Result<()> {
    if std::env::var("FAIRWAY_INSTALLER_TEST").as_deref() != Ok("1")
        || std::env::var("GITHUB_ACTIONS").as_deref() != Ok("true")
    {
        return Err("This probe modifies a disposable CI account. Run it only through installer_lifecycle.py on a GitHub-hosted runner.".into());
    }
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.len() != 3 || !["seed", "verify"].contains(&args[0].as_str()) {
        return Err("Usage: installer_probe seed|verify INSTALLED_RUNTIME STATE_FILE".into());
    }
    let layout = Layout::new(PathBuf::from(&args[1]))?;
    let state_path = PathBuf::from(&args[2]);
    if args[0] == "seed" && (state_path.exists() || layout.data.join("database-ready").exists()) {
        return Err(
            "Installer test requires a fresh disposable account; existing database was preserved."
                .into(),
        );
    }
    let mut manager = Manager::new(layout)?;
    let seed = args[0] == "seed";
    let coursework = manager.layout.www.join("beta-installer-check.php");
    let result = (|| -> Result<()> {
        if seed {
            let listeners = (0..4)
                .map(|_| TcpListener::bind(("127.0.0.1", 0)))
                .collect::<std::io::Result<Vec<_>>>()
                .map_err(|e| e.to_string())?;
            let ports = listeners
                .iter()
                .map(|listener| listener.local_addr().unwrap().port())
                .collect::<Vec<_>>();
            let ports = Ports {
                website: ports[0],
                phpmyadmin: ports[1],
                mysql: ports[2],
                php: ports[3],
            };
            drop(listeners);
            manager.save_ports(ports)?;
            fs::write(&coursework, "<?php echo 'Student May coursework retained';")
                .map_err(|e| e.to_string())?;
        } else {
            let state: serde_json::Value =
                serde_json::from_slice(&fs::read(&state_path).map_err(|e| e.to_string())?)
                    .map_err(|e| e.to_string())?;
            assert_eq!(
                serde_json::to_value(&manager.ports).unwrap(),
                state["ports"]
            );
            assert_eq!(
                manager.layout.php_version()?,
                state["phpVersion"].as_str().unwrap()
            );
            assert!(PathBuf::from(state["backup"].as_str().unwrap()).is_file());
            assert_eq!(
                fs::read_to_string(&coursework).map_err(|e| e.to_string())?,
                "<?php echo 'Student May coursework retained';"
            );
        }
        manager.start_all()?;
        assert!(manager
            .snapshot()
            .services
            .iter()
            .all(|service| service.ready));
        let mysql = || {
            let mut command = process::command(manager.layout.exe("mysql", "mysql"));
            command
                .arg(format!(
                    "--defaults-file={}",
                    manager.layout.data.join("config/admin.cnf").display()
                ))
                .args(["--batch", "--skip-column-names"]);
            command
        };
        if seed {
            process::sql(mysql(), "CREATE DATABASE beta_lifecycle; CREATE TABLE beta_lifecycle.coursework (id INT PRIMARY KEY, title VARCHAR(100)); INSERT INTO beta_lifecycle.coursework VALUES (1,'Student May coursework retained');")?;
        }
        let mut query = mysql();
        query.args([
            "-e",
            "SELECT title FROM beta_lifecycle.coursework WHERE id=1;",
        ]);
        assert_eq!(process::output(query)?, "Student May coursework retained");
        if seed {
            let backup = manager.backup()?;
            fs::write(&state_path, serde_json::to_vec_pretty(&serde_json::json!({
                "ports": manager.ports, "phpVersion": manager.layout.php_version()?, "backup": backup,
                "documentRoot": manager.layout.www, "dataDir": manager.layout.data,
            })).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
        }
        for command in ["php", "composer"] {
            let path = fairway_core::integration::launcher(&manager.layout, command)
                .ok_or("Installed command launcher is missing")?;
            let mut command = process::command(path);
            command.arg("--version");
            process::output(command)?;
        }
        println!("{}", diagnostics::report(&manager.snapshot()));
        Ok(())
    })();
    let stop = manager.stop_all();
    result.and(stop)
}

fn main() -> Result<()> {
    run()
}
