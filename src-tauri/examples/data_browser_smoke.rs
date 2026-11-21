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

fn schema_details(layout: &Layout, database: &str) -> Result<(), String> {
    let query = |sql: &str| call(layout, json!({"op":"query","database":database,"sql":sql}));
    let structure = |table: &str| {
        call(
            layout,
            json!({"op":"structure","database":database,"table":table}),
        )
    };
    let index = |table: &str, name: &str| -> Result<Value, String> {
        structure(table)?["indexes"]
            .as_array()
            .unwrap()
            .iter()
            .find(|index| index["name"] == name)
            .cloned()
            .ok_or_else(|| format!("Missing index {name}"))
    };
    let drop_index = |name: &str, original: &Value| {
        call(
            layout,
            json!({"op":"drop_index","database":database,"table":"schema_memberships","name":name,"originalIndex":original,"confirmed":true}),
        )
    };
    query(&format!("CREATE TABLE schema_teams(id INT PRIMARY KEY); CREATE TABLE schema_memberships(id INT PRIMARY KEY, team_id INT, email VARCHAR(80) NOT NULL, term VARCHAR(20) NOT NULL, CONSTRAINT fk_schema_team FOREIGN KEY(team_id) REFERENCES schema_teams(id) ON UPDATE RESTRICT ON DELETE CASCADE); INSERT INTO schema_memberships VALUES(1,NULL,'same','a'),(2,NULL,'same','b'),(3,NULL,'third','b'); CREATE DATABASE browser_schema_other; CREATE TABLE browser_schema_other.memberships(team_id INT, FOREIGN KEY(team_id) REFERENCES {database}.schema_teams(id));"))?;
    let create_index = |name: &str, columns: Value, unique: bool| {
        call(
            layout,
            json!({"op":"create_index","database":database,"table":"schema_memberships","name":name,"indexColumns":columns,"unique":unique}),
        )
    };
    let duplicate = create_index("uq_schema_email", json!(["email"]), true).unwrap_err();
    assert!(
        duplicate.contains("existing values contain duplicates"),
        "{duplicate}"
    );
    assert!(!structure("schema_memberships")?["indexes"]
        .as_array()
        .unwrap()
        .iter()
        .any(|index| index["name"] == "uq_schema_email"));
    create_index("uq_schema_term_email", json!(["term", "email"]), true)?;
    let unique = index("schema_memberships", "uq_schema_term_email")?;
    assert_eq!(unique["unique"], true);
    assert_eq!(unique["parts"][0]["name"], "term");
    assert_eq!(unique["parts"][1]["name"], "email");
    assert!(query("INSERT INTO schema_memberships VALUES(4,NULL,'same','a')").is_err());

    let child = structure("schema_memberships")?;
    let foreign = &child["foreignKeys"][0];
    assert_eq!(foreign["columns"], json!(["team_id"]));
    assert_eq!(foreign["referencedDatabase"], database);
    assert_eq!(foreign["referencedTable"], "schema_teams");
    assert_eq!(foreign["referencedColumns"], json!(["id"]));
    assert_eq!(foreign["onUpdate"], "RESTRICT");
    assert_eq!(foreign["onDelete"], "CASCADE");
    let parent = structure("schema_teams")?;
    assert!(parent["referencedBy"].as_array().unwrap().contains(foreign));
    assert!(parent["referencedBy"]
        .as_array()
        .unwrap()
        .iter()
        .any(|link| link["database"] == "browser_schema_other" && link["table"] == "memberships"));
    for table in ["schema_teams", "schema_memberships"] {
        let protected = index(
            table,
            if table == "schema_teams" {
                "PRIMARY"
            } else {
                "fk_schema_team"
            },
        )?;
        assert_eq!(protected["removable"], false);
        let error = call(layout, json!({"op":"drop_index","database":database,"table":table,"name":protected["name"],"originalIndex":protected,"confirmed":true})).unwrap_err();
        assert!(error.contains("manage it in SQL"), "{error}");
    }
    assert_eq!(index("schema_memberships", "PRIMARY")?["removable"], false);

    create_index("idx_schema_ordinary", json!(["term"]), false)?;
    let old = index("schema_memberships", "idx_schema_ordinary")?;
    query("DROP INDEX idx_schema_ordinary ON schema_memberships; CREATE INDEX idx_schema_ordinary ON schema_memberships(email)")?;
    let stale = drop_index("idx_schema_ordinary", &old).unwrap_err();
    assert!(stale.contains("changed or was removed"), "{stale}");
    let current = index("schema_memberships", "idx_schema_ordinary")?;
    assert_eq!(current["parts"][0]["name"], "email");
    assert!(call(layout, json!({"op":"drop_index","database":database,"table":"schema_memberships","name":"idx_schema_ordinary","originalIndex":current})).unwrap_err().contains("Confirm"));
    let backups = || -> Result<Vec<PathBuf>, String> {
        std::fs::read_dir(layout.data.join("backups"))
            .map_err(|e| e.to_string())?
            .map(|entry| entry.map(|entry| entry.path()).map_err(|e| e.to_string()))
            .collect()
    };
    let before = backups()?;
    drop_index("idx_schema_ordinary", &current)?;
    assert!(index("schema_memberships", "idx_schema_ordinary").is_err());
    let published: Vec<_> = backups()?
        .into_iter()
        .filter(|path| !before.contains(path))
        .collect();
    assert_eq!(published.len(), 1);
    assert_eq!(
        published[0].extension().and_then(|ext| ext.to_str()),
        Some("sql")
    );
    assert!(std::fs::read_to_string(&published[0])
        .map_err(|e| e.to_string())?
        .contains("KEY `idx_schema_ordinary`"));
    query("DROP DATABASE browser_schema_other; DROP TABLE schema_memberships; DROP TABLE schema_teams")?;
    println!("PASS: ordered MySQL indexes, duplicate and stale guards, protected keys, cross-schema relationships and backup-before-removal.");
    Ok(())
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
    let listeners: Vec<_> = (0..6)
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
        mail: ports[4],
        smtp: ports[5],
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
            schema_details(l, db)?;
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
            let counts = call(
                l,
                json!({"op":"query","database":db,"sql":"CREATE TABLE update_counts(id INT PRIMARY KEY, value INT); INSERT INTO update_counts VALUES(1,10),(2,20); UPDATE update_counts SET value=10 WHERE id=1; UPDATE update_counts SET value=20; UPDATE update_counts SET value=20; DROP TABLE update_counts;"}),
            )?;
            assert_eq!(counts["sets"][2]["affected"], 0);
            assert_eq!(counts["sets"][3]["affected"], 1);
            assert_eq!(counts["sets"][4]["affected"], 0);
            let limited = call(
                l,
                json!({"op":"query","database":db,"sql":"WITH RECURSIVE n AS (SELECT 1 x UNION ALL SELECT x+1 FROM n WHERE x<250) SELECT x FROM n LIMIT 250; SELECT 42 AS answer;"}),
            )?;
            assert_eq!(limited["sets"][0]["rows"].as_array().unwrap().len(), 200);
            assert_eq!(limited["sets"][0]["truncated"], true);
            assert_eq!(limited["sets"][1]["rows"][0][0], "42");
            // Per-request connections must never report uncommitted work as saved.
            call(
                l,
                json!({"op":"query","database":db,"sql":"CREATE TABLE transactions(id INT PRIMARY KEY, value TEXT); INSERT INTO transactions VALUES(1,'initial')"}),
            )?;
            for sql in [
                "START TRANSACTION; UPDATE transactions SET value='lost' WHERE id=1; SELECT * FROM transactions",
                "SET autocommit=0; UPDATE transactions SET value='lost' WHERE id=1",
                "START TRANSACTION; UPDATE transactions SET value='lost' WHERE id=1; SELECT * FROM missing_table",
            ] {
                let error = call(l, json!({"op":"query","database":db,"sql":sql})).unwrap_err();
                assert!(error.contains("rolled back"), "{error}");
                assert_eq!(call(l, json!({"op":"query","database":db,"sql":"SELECT value FROM transactions"}))?["sets"][0]["rows"][0][0], "initial");
            }
            call(
                l,
                json!({"op":"query","database":db,"sql":"START TRANSACTION; UPDATE transactions SET value='committed' WHERE id=1; COMMIT"}),
            )?;
            assert_eq!(
                call(
                    l,
                    json!({"op":"query","database":db,"sql":"SELECT value FROM transactions"})
                )?["sets"][0]["rows"][0][0],
                "committed"
            );
            call(
                l,
                json!({"op":"query","database":db,"sql":"START TRANSACTION; UPDATE transactions SET value='discarded' WHERE id=1; ROLLBACK"}),
            )?;
            let error = call(l, json!({"op":"query","database":db,"sql":"START TRANSACTION; UPDATE transactions SET value='kept after commit' WHERE id=1; COMMIT; SELECT * FROM missing_table"})).unwrap_err();
            assert!(error.contains("Earlier committed") && !error.contains("rolled back"));
            assert_eq!(
                call(
                    l,
                    json!({"op":"query","database":db,"sql":"SELECT value FROM transactions"})
                )?["sets"][0]["rows"][0][0],
                "kept after commit"
            );
            let error = call(l, json!({"op":"query","database":db,"sql":"UPDATE transactions SET value='kept' WHERE id=1; START TRANSACTION; UPDATE transactions SET value='lost' WHERE id=1; SELECT * FROM missing_table"})).unwrap_err();
            assert!(error.contains("Earlier committed") && error.contains("rolled back"));
            assert_eq!(
                call(
                    l,
                    json!({"op":"query","database":db,"sql":"SELECT value FROM transactions"})
                )?["sets"][0]["rows"][0][0],
                "kept"
            );
            // Ten is a display cap: all requested writes/commits run and later errors surface.
            call(
                l,
                json!({"op":"query","database":db,"sql":"CREATE TABLE counter(value INT)"}),
            )?;
            let many = call(
                l,
                json!({"op":"query","database":db,"sql":"INSERT INTO counter VALUES(1);".repeat(11)}),
            )?;
            assert_eq!(many["sets"].as_array().unwrap().len(), 10);
            assert_eq!(many["resultsTruncated"], true);
            assert_eq!(
                call(
                    l,
                    json!({"op":"query","database":db,"sql":"SELECT COUNT(*) FROM counter"})
                )?["sets"][0]["rows"][0][0],
                "11"
            );
            call(
                l,
                json!({"op":"query","database":db,"sql":format!("START TRANSACTION; {} COMMIT", "INSERT INTO counter VALUES(2);".repeat(11))}),
            )?;
            assert_eq!(
                call(
                    l,
                    json!({"op":"query","database":db,"sql":"SELECT COUNT(*) FROM counter"})
                )?["sets"][0]["rows"][0][0],
                "22"
            );
            let error = call(l, json!({"op":"query","database":db,"sql":format!("{} SELECT * FROM missing_table", "SELECT 1;".repeat(11))})).unwrap_err();
            assert!(error.contains("missing_table"));
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
            call(
                l,
                json!({"op":"create_database","database":"browser_ui_import"}),
            )?;
            let import_request = json!({"op":"import","database":"browser_ui_import","sqlFile":dump,"confirmed":true});
            let mut unconfirmed = import_request.clone();
            unconfirmed["confirmed"] = json!(false);
            assert!(call(l, unconfirmed).unwrap_err().contains("Confirm"));
            let imported = call(l, import_request.clone())?;
            assert!(PathBuf::from(imported["backupPath"].as_str().unwrap()).is_file());
            assert_eq!(
                call(
                    l,
                    json!({"op":"rows","database":"browser_ui_import","table":"students"})
                )?["rows"][0]["id"],
                "9007199254740993"
            );
            let canceled = Arc::new(AtomicBool::new(true));
            assert!(data_browser::execute(l, import_request, canceled)
                .unwrap_err()
                .contains("canceled"));

            // Full client dumps support DELIMITER while shell commands remain disabled.
            let script = l.data.join("import-procedure.sql");
            std::fs::write(&script, "DELIMITER $$\nCREATE PROCEDURE imported_proc() BEGIN SELECT 1; END$$\nDELIMITER ;\nCALL imported_proc();\n").map_err(|e| e.to_string())?;
            call(
                l,
                json!({"op":"import","database":"browser_ui_import","sqlFile":script,"confirmed":true}),
            )?;
            let invalid = l.data.join("partial-import.sql");
            std::fs::write(&invalid, "CREATE TABLE partial_import (id INT); INSERT INTO partial_import VALUES (1); INVALID SQL;").map_err(|e| e.to_string())?;
            let error = call(l, json!({"op":"import","database":"browser_ui_import","sqlFile":invalid,"confirmed":true})).unwrap_err();
            assert!(
                error.contains("earlier statements may have completed")
                    && error.contains("Backup saved:")
            );
            assert_eq!(
                call(
                    l,
                    json!({"op":"rows","database":"browser_ui_import","table":"partial_import"})
                )?["total"],
                "1"
            );

            // A failed protective backup must prevent any import statement.
            let backup_folder = l.data.join("backups");
            let kept_backups = l.data.join("backups-kept");
            std::fs::rename(&backup_folder, &kept_backups).map_err(|e| e.to_string())?;
            std::fs::write(&backup_folder, "block backup creation").map_err(|e| e.to_string())?;
            let guarded = call(
                l,
                json!({"op":"import","database":"browser_ui_import","sqlFile":script,"confirmed":true}),
            );
            std::fs::remove_file(&backup_folder).map_err(|e| e.to_string())?;
            std::fs::rename(&kept_backups, &backup_folder).map_err(|e| e.to_string())?;
            assert!(guarded.unwrap_err().contains("Nothing was imported"));
            let slow = l.data.join("cancel-import.sql");
            std::fs::write(
                &slow,
                "SELECT SLEEP(10); CREATE TABLE must_not_import (id INT);",
            )
            .map_err(|e| e.to_string())?;
            let cancel_import = Arc::new(AtomicBool::new(false));
            let signal = cancel_import.clone();
            let timer_import = thread::spawn(move || {
                thread::sleep(Duration::from_millis(500));
                signal.store(true, Ordering::SeqCst);
            });
            let canceled = data_browser::execute(l, json!({"op":"import","database":"browser_ui_import","sqlFile":slow,"confirmed":true}), cancel_import).unwrap_err();
            timer_import.join().unwrap();
            assert!(canceled.contains("canceled") && canceled.contains("Backup saved:"));
            assert!(
                !call(l, json!({"op":"tables","database":"browser_ui_import"}))?["tables"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|table| table["name"] == "must_not_import")
            );
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
            call(
                l,
                json!({"op":"query","database":db,"sql":"DROP DATABASE browser_ui_import; DROP DATABASE browser_ready; DROP DATABASE fairway;"}),
            )?;
            let empty = manager.backup()?;
            assert!(std::fs::read_to_string(empty)
                .map_err(|e| e.to_string())?
                .contains("no student databases"));
            fairway_core::updates::prepare(&mut manager)?;
            assert!(manager
                .snapshot()
                .services
                .iter()
                .all(|service| !service.running));
            println!("PASS: empty-database backup and updater preparation do not block upgrades.");
            println!("PASS: real MySQL database/table/column operations, exact BIGINT/DECIMAL, quotes/Unicode, NULL vs empty/default, stale edits, no-key/view/composite-key behavior, multi-result SQL, cancellation, SQL export/import restore, confirmation and system-schema guards.");
            Ok(())
        },
    ));
    let stopped = manager.stop_all();
    result
        .map_err(|_| "Database smoke test assertion failed.".to_string())?
        .and(stopped)
}
