//! Real database coverage in an isolated directory, never a student's active database.
use fairway_core::{
    config::{Layout, Ports},
    data_browser,
    manager::Manager,
};
use serde_json::{json, Value};
use std::{
    net::TcpListener,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    thread,
    time::Duration,
};

fn call(layout: &Layout, request: Value) -> Result<Value, String> {
    data_browser::execute(layout, request, Arc::new(AtomicBool::new(false)))
}
fn main() -> Result<(), String> {
    let root = std::env::current_dir()
        .map_err(|e| e.to_string())?
        .join(PathBuf::from(
            std::env::args()
                .nth(1)
                .ok_or("Provide an isolated test directory.")?,
        ));
    if root.join("data").exists() {
        return Err("Use a fresh test directory.".into());
    }
    let layout = Layout::at(
        root.join("Student May's home 日本"),
        root.join("data"),
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("resources/runtime"),
    );
    let mut manager = Manager::new(layout)?;
    let listeners: Vec<_> = (0..4)
        .map(|_| TcpListener::bind(("127.0.0.1", 0)).unwrap())
        .collect();
    let ports: Vec<_> = listeners
        .iter()
        .map(|l| l.local_addr().unwrap().port())
        .collect();
    manager.save_ports(Ports {
        website: ports[0],
        phpmyadmin: ports[1],
        mysql: ports[2],
        php: ports[3],
    })?;
    drop(listeners);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> Result<(), String> {
            manager.start("mysql")?;
            let l = &manager.layout;
            call(l, json!({"op":"create_database","database":"browser_test"}))?;
            call(
                l,
                json!({"op":"rename_database","database":"browser_test","name":"browser_ready","confirmed":true}),
            )?;
            let db = "browser_ready";
            call(
                l,
                json!({"op":"create_table","database":db,"table":"students","columns":[
                    {"name":"id","type":"BIGINT","unsigned":true,"primary":true,"autoIncrement":true},
                    {"name":"name","type":"VARCHAR","length":100,"nullable":false},
                    {"name":"note","type":"TEXT","nullable":true},
                    {"name":"amount","type":"DECIMAL","length":20,"scale":4,"defaultMode":"value","defaultValue":"12.5000"}
                ]}),
            )?;
            let name = "Student May's `name`; DROP DATABASE browser_ready; -- 🟢";
            call(
                l,
                json!({"op":"insert","database":db,"table":"students","values":{"id":"9007199254740993","name":name,"note":null}}),
            )?;
            let first = call(l, json!({"op":"rows","database":db,"table":"students"}))?;
            assert_eq!(first["rows"][0]["id"], "9007199254740993");
            assert_eq!(first["rows"][0]["amount"], "12.5000");
            assert_eq!(first["rows"][0]["name"], name);
            assert!(first["rows"][0]["note"].is_null());
            let original = first["rows"][0].clone();
            call(
                l,
                json!({"op":"update","database":db,"table":"students","original":original,"values":{"note":""}}),
            )?;
            assert!(call(l, json!({"op":"update","database":db,"table":"students","original":original,"values":{"name":"stale"}})).is_err());
            let current =
                call(l, json!({"op":"rows","database":db,"table":"students"}))?["rows"][0].clone();
            assert_eq!(current["note"], "");
            // A no-op save still matches one row; case-only concurrent changes must conflict.
            call(
                l,
                json!({"op":"update","database":db,"table":"students","original":current,"values":{"name":name}}),
            )?;
            call(
                l,
                json!({"op":"query","database":db,"sql":"UPDATE students SET name=UPPER(name)"}),
            )?;
            assert!(call(
                l,
                json!({"op":"delete","database":db,"table":"students","original":current})
            )
            .is_err());
            let filter = call(
                l,
                json!({"op":"rows","database":db,"table":"students","filterColumn":"name","filterValue":"DROP DATABASE"}),
            )?;
            assert_eq!(filter["total"], "1");
            let sets = call(
                l,
                json!({"op":"query","database":db,"sql":"SELECT 9007199254740993 AS big; SELECT NULL AS n, '' AS empty_value;"}),
            )?;
            assert_eq!(sets["sets"][0]["rows"][0][0], "9007199254740993");
            assert!(sets["sets"][1]["rows"][0][0].is_null());
            let limited = call(
                l,
                json!({"op":"query","database":db,"sql":"WITH RECURSIVE n AS (SELECT 1 x UNION ALL SELECT x+1 FROM n WHERE x<250) SELECT x FROM n LIMIT 250; SELECT 42 AS answer;"}),
            )?;
            assert_eq!(limited["sets"][0]["rows"].as_array().unwrap().len(), 200);
            assert_eq!(limited["sets"][0]["truncated"], true);
            assert_eq!(limited["sets"][1]["rows"][0][0], "42");
            call(
                l,
                json!({"op":"query","database":db,"sql":"CREATE TABLE no_key (message TEXT); INSERT INTO no_key VALUES ('hello'); CREATE VIEW student_names AS SELECT id,name FROM students; CREATE TABLE composite (a INT,b INT, PRIMARY KEY(a,b)); INSERT INTO composite VALUES(1,2);"}),
            )?;
            let no_key = call(l, json!({"op":"rows","database":db,"table":"no_key"}))?;
            assert_eq!(no_key["key"], json!([]));
            assert!(call(
                l,
                json!({"op":"delete","database":db,"table":"no_key","original":no_key["rows"][0]})
            )
            .is_err());
            assert_eq!(
                call(
                    l,
                    json!({"op":"rows","database":db,"table":"student_names"})
                )?["kind"],
                "VIEW"
            );
            assert_eq!(
                call(
                    l,
                    json!({"op":"structure","database":db,"table":"composite"})
                )?["key"],
                json!(["a", "b"])
            );
            let pair =
                call(l, json!({"op":"rows","database":db,"table":"composite"}))?["rows"][0].clone();
            call(
                l,
                json!({"op":"delete","database":db,"table":"composite","original":pair}),
            )?;
            assert!(call(
            l,
            json!({"op":"rename_database","database":db,"name":"must_not_rename","confirmed":true})
        )
        .is_err());
            assert!(call(l, json!({"op":"drop_database","database":db})).is_err());
            assert!(call(l, json!({"op":"create_database","database":"mysql"})).is_err());
            call(
                l,
                json!({"op":"add_column","database":db,"table":"students","column":{"name":"active","type":"BOOLEAN","defaultMode":"value","defaultValue":"1"}}),
            )?;
            call(
                l,
                json!({"op":"change_column","database":db,"table":"students","oldName":"active","column":{"name":"enabled","type":"INT","defaultMode":"value","defaultValue":"1"},"confirmed":true}),
            )?;
            call(
                l,
                json!({"op":"drop_column","database":db,"table":"students","name":"enabled","confirmed":true}),
            )?;
            call(
                l,
                json!({"op":"query","database":db,"sql":"ALTER TABLE students MODIFY name VARCHAR(100) CHARACTER SET utf8mb4 COLLATE utf8mb4_bin NOT NULL COMMENT 'Keep this comment'"}),
            )?;
            call(
                l,
                json!({"op":"change_column","database":db,"table":"students","oldName":"name","column":{"name":"name","type":"VARCHAR","length":120,"nullable":false},"confirmed":true}),
            )?;
            let changed = call(
                l,
                json!({"op":"structure","database":db,"table":"students"}),
            )?;
            assert_eq!(changed["columns"][1]["comment"], "Keep this comment");
            assert_eq!(changed["columns"][1]["collation"], "utf8mb4_bin");
            call(
                l,
                json!({"op":"rename_table","database":db,"table":"no_key","name":"notes"}),
            )?;
            let dump = data_browser::export(l, db, None, None, "all")?;
            // Windows cannot unlink an export while its file handle is open.
            assert!(std::fs::read_dir(l.data.join("backups"))
                .unwrap()
                .all(|entry| !entry
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "partial")));
            assert!(
                data_browser::export(l, db, None, Some(&dump), "all").is_err(),
                "Must never overwrite a dump"
            );
            call(
                l,
                json!({"op":"create_database","database":"browser_restored"}),
            )?;
            data_browser::import(l, "browser_restored", &dump)?;
            let restored = call(
                l,
                json!({"op":"rows","database":"browser_restored","table":"students"}),
            )?;
            assert_eq!(restored["rows"][0]["id"], "9007199254740993");
            assert_eq!(restored["rows"][0]["amount"], "12.5000");
            assert_eq!(restored["rows"][0]["note"], "");
            let cancel = Arc::new(AtomicBool::new(false));
            let signal = cancel.clone();
            let timer = thread::spawn(move || {
                thread::sleep(Duration::from_millis(500));
                signal.store(true, Ordering::SeqCst);
            });
            assert!(data_browser::execute(
                l,
                json!({"op":"query","database":db,"sql":"SELECT SLEEP(10)"}),
                cancel
            )
            .unwrap_err()
            .contains("canceled"));
            timer.join().unwrap();
            call(l, json!({"op":"query","database":db,"sql":"SELECT 1"}))?;
            call(
                l,
                json!({"op":"drop_table","database":db,"table":"notes","confirmed":true}),
            )?;
            call(
                l,
                json!({"op":"drop_database","database":"browser_restored","confirmed":true}),
            )?;
            println!("PASS: real MySQL database/table/column operations, exact BIGINT/DECIMAL, quotes/Unicode, NULL vs empty/default, stale edits, no-key/view/composite-key behavior, multi-result SQL, cancellation, SQL export/import restore, confirmation and system-schema guards.");
            Ok(())
        },
    ));
    let stopped = manager.stop_all();
    result
        .map_err(|_| "Database smoke test assertion failed.".to_string())?
        .and(stopped)
}
