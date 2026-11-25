use super::*;
use std::{
    collections::BTreeMap,
    fs,
    net::TcpListener,
    os::unix::fs::{symlink, PermissionsExt},
};
#[derive(Default)]
struct MemoryVault(Mutex<BTreeMap<String, Secret>>);
impl Vault for MemoryVault {
    fn create(&self, id: &str, s: &Secret) -> Result<()> {
        let mut m = self.0.lock().unwrap();
        if m.contains_key(id) {
            return Err(failure("Duplicate fixture secret"));
        }
        m.insert(id.into(), Secret(s.0.clone()));
        Ok(())
    }
    fn read(&self, id: &str) -> Result<Secret> {
        self.0
            .lock()
            .unwrap()
            .get(id)
            .map(|s| Secret(s.0.clone()))
            .ok_or_else(|| failure("Fixture secret unavailable"))
    }
}
struct Scratch(PathBuf);
impl Scratch {
    fn new() -> Self {
        let p = std::env::temp_dir().join(format!("fw-pg2-{:032x}", rand::random::<u128>()));
        fs::create_dir(&p).unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o700)).unwrap();
        Self(fs::canonicalize(p).unwrap())
    }
}
impl Drop for Scratch {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("Retained isolated PostgreSQL fixture: {}", self.0.display());
            return;
        }
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn fixture() -> (Scratch, Controller, Arc<MemoryVault>) {
    let s = Scratch::new();
    let v = Arc::new(MemoryVault::default());
    let l = Layout::at(s.0.join("home"), s.0.clone(), s.0.join("tools"));
    let c = Controller::open(&l, v.clone()).unwrap();
    (s, c, v)
}
#[test]
fn bundle_source_reading_does_not_relax_private_managed_storage() {
    let s = Scratch::new();
    fs::set_permissions(&s.0, fs::Permissions::from_mode(0o755)).unwrap();
    let dir = Dir::absolute(&s.0).unwrap();
    assert!(dir.private().is_err());
    dir.bundle_directory().unwrap();
    let path = s.0.join("source");
    fs::write(&path, b"pinned bytes").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    assert_eq!(dir.read_bundle("source", 100).unwrap(), b"pinned bytes");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    assert!(dir.read_bundle("source", 100).is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o444)).unwrap();
    fs::hard_link(&path, s.0.join("alias")).unwrap();
    assert!(dir.read_bundle("source", 100).is_err());
    fs::remove_file(s.0.join("alias")).unwrap();
    symlink(&path, s.0.join("link")).unwrap();
    assert!(dir.read_bundle("link", 100).is_err());
    fs::set_permissions(&s.0, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(dir.bundle_directory().is_err());
}
#[test]
fn private_operations_refuse_links_foreign_publication_and_changed_identity() {
    let s = Scratch::new();
    let d = Dir::absolute(&s.0).unwrap();
    d.private().unwrap();
    let stage = d.make("stage").unwrap();
    stage.create("sentinel", b"kept", false).unwrap();
    d.make("occupied").unwrap();
    assert!(d.publish("stage", "occupied").is_err());
    assert_eq!(stage.read("sentinel", 100).unwrap(), b"kept");
    symlink(s.0.join("stage"), s.0.join("link")).unwrap();
    assert!(d.child("link").is_err());
    assert!(d.read("link", 100).is_err());
    fs::rename(s.0.join("stage"), s.0.join("old")).unwrap();
    d.make("stage").unwrap();
    assert!(stage.assert_path(&s.0.join("stage")).is_err());
    let lease = d.lock("lease").unwrap();
    assert!(d.lock("lease").is_err());
    drop(lease);
    d.lock("lease").unwrap();
    fs::hard_link(s.0.join("old/sentinel"), s.0.join("hardlink")).unwrap();
    assert!(d.read("hardlink", 100).is_err());
    assert!(d.entries().is_err());
}
#[test]
fn consent_constructor_and_read_only_requests_never_initialize_data() {
    let s = Scratch::new();
    let layout = Layout::at(s.0.join("home"), s.0.clone(), s.0.join("tools"));
    let m = Managed::new(Some(layout));
    assert!(m
        .initialize(Initialize {
            confirmed: false,
            port: 15432
        })
        .is_err());
    assert!(!s.0.join("postgresql").exists());
    assert!(m.provision(Confirmation { confirmed: false }).is_err());
    assert!(!m.snapshot().unwrap().running);
    assert!(!m.health().unwrap().running);
    assert!(m.logs().unwrap().text.is_empty());
    assert!(!s.0.join("postgresql").exists());
}
#[test]
fn foreign_root_and_reserved_ports_are_refused_without_shared_settings_migration() {
    let s = Scratch::new();
    let l = Layout::at(s.0.join("home"), s.0.clone(), s.0.join("tools"));
    let data = Dir::absolute(&s.0).unwrap();
    let foreign = data.make("postgresql").unwrap();
    foreign.create("PG_VERSION", b"18\n", false).unwrap();
    assert!(Controller::open(&l, Arc::new(MemoryVault::default())).is_err());
    assert!(!foreign.exists("service.lock").unwrap());
    assert_eq!(foreign.read("PG_VERSION", 100).unwrap(), b"18\n");
    let (_s, c, _) = fixture();
    assert!(c.check_port(13306).is_err());
    assert!(c.check_port(crate::tls::https_port()).is_err());
    let old = br#"{"website":8800,"phpmyadmin":8801,"mysql":13306,"php":19000}"#;
    fs::write(c.settings.join("settings.json"), old).unwrap();
    assert!(c.check_port(13306).is_err());
    assert_eq!(fs::read(c.settings.join("settings.json")).unwrap(), old);
}
#[test]
fn malformed_pin_and_existing_unowned_runtime_are_never_adopted() {
    let (_s, c, _v) = fixture();
    c.base.make("runtime-18.6").unwrap();
    assert!(c.runtime().is_err());
    assert!(runtime::provision(&c.base, &c.path, &c.source).is_err());
}
#[test]
fn bounded_logs_never_retain_unbounded_output() {
    let s = Scratch::new();
    let file = files::log_file(&s.0.join("log")).unwrap();
    let mut cap = logs::Capture::new(file);
    cap.test_append(&vec![b'x'; 2 * 1024 * 1024]);
    let out = cap.read();
    assert!(out.truncated);
    assert_eq!(out.text.len(), 64 * 1024);
    assert!(fs::metadata(s.0.join("log")).unwrap().len() <= 1024 * 1024);
}
#[test]
#[ignore = "Requires the explicitly provisioned pinned payload; uses memory-only fixture vault, never login Keychain"]
fn real_pinned_runtime_scram_lifecycle_health_integrity_and_independence() {
    let (_s, mut c, vault) = fixture();
    c.source = std::env::var_os("FAIRWAY_TEST_POSTGRES_PAYLOAD")
        .expect("Provide isolated pinned payload")
        .into();
    runtime::provision(&c.base, &c.path, &c.source).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let occupied = listener.local_addr().unwrap().port();
    assert!(c.initialize(occupied).is_err());
    let port = {
        let l = TcpListener::bind("127.0.0.1:0").unwrap();
        l.local_addr().unwrap().port()
    };
    struct DeniedVault;
    impl Vault for DeniedVault {
        fn create(&self, _: &str, _: &Secret) -> Result<()> {
            Err(failure("Fixture vault unavailable"))
        }
        fn read(&self, _: &str) -> Result<Secret> {
            Err(failure("Fixture vault unavailable"))
        }
    }
    let (_denied_scratch, mut denied, _) = fixture();
    denied.source = c.source.clone();
    runtime::provision(&denied.base, &denied.path, &denied.source).unwrap();
    denied.vault = Arc::new(DeniedVault);
    assert!(denied.initialize(port).is_err());
    let entries = denied.base.entries().unwrap();
    assert!(!entries
        .iter()
        .any(|(n, _)| n.starts_with(".cluster-") || n.starts_with("cluster-")));
    let job = entries.iter().find(|(n, _)| n.starts_with("job-")).unwrap();
    let intent: serde_json::Value = decode(&denied.base.read(&job.0, 4096).unwrap()).unwrap();
    assert_eq!(intent["phase"], "securingCredential");
    denied.vault = vault.clone();
    assert!(denied.initialize(port).is_err()); // Retained intent never auto-retried.
    c.initialize(port).unwrap();
    assert!(c.run.is_none());
    let id = c.cluster.as_ref().unwrap().id.clone();
    let path = c.cluster_path(c.cluster.as_ref().unwrap()).unwrap();
    assert_eq!(fs::read(path.join("PG_VERSION")).unwrap(), b"18\n");
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o700
    );
    assert!(c.initialize(port).is_err());
    c.start().unwrap();
    // A private worker must ignore inherited libpq service configuration.
    std::env::set_var("PGSERVICE", "fairway-fixture-must-never-be-loaded");
    let status = c.snapshot(true).unwrap();
    std::env::remove_var("PGSERVICE");
    assert_eq!(status.authentication, "authenticated");
    assert_eq!(status.responsiveness, "accepting");
    assert!(listener.local_addr().is_ok());
    assert!(c.base.lock("runtime.lease").is_err());
    assert!(c.start().is_err());
    let secret = {
        let mut values = vault.0.lock().unwrap();
        let (_, value) = values.iter_mut().next().unwrap();
        let old = Secret(value.0.clone());
        *value = Secret::random();
        old
    };
    let rejected = c.snapshot(true).unwrap();
    assert_eq!(rejected.responsiveness, "accepting");
    assert_eq!(rejected.authentication, "rejected");
    {
        let mut values = vault.0.lock().unwrap();
        *values.values_mut().next().unwrap() = Secret(secret.0.clone());
    }
    // A real authenticated client holds a session. Smart shutdown must wait,
    // retain ownership/lease and avoid a forced database kill.
    use std::io::Write;
    let (read, mut write) = std::os::unix::net::UnixStream::pair().unwrap();
    let fd: std::os::fd::OwnedFd = read.into();
    write.write_all(&secret.0).unwrap();
    write.write_all(b"\n").unwrap();
    write.shutdown(std::net::Shutdown::Write).unwrap();
    let rt = c.runtime().unwrap();
    let mut client = clean_command(rt.path.join("bin/psql"));
    let mut client = client
        .args([
            "-X",
            "-W",
            "-h",
            "127.0.0.1",
            "-U",
            "fairway",
            "-d",
            "postgres",
            "-p",
        ])
        .arg(port.to_string())
        .args(["-c", "SELECT pg_sleep(15)"])
        .stdin(Stdio::from(fd))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    drop(write);
    std::thread::sleep(Duration::from_millis(750));
    assert!(client.try_wait().unwrap().is_none());
    assert!(
        c.stop().is_err(),
        "Smart shutdown must wait for the authenticated fixture client"
    );
    assert!(c.run.is_some());
    assert!(c.base.lock("runtime.lease").is_err());
    // Let the bounded query complete normally; closing a client socket alone
    // does not cancel a running PostgreSQL statement.
    client.wait().unwrap();
    c.stop().unwrap();
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
    c.start().unwrap();
    assert_eq!(c.snapshot(true).unwrap().authentication, "authenticated");
    c.stop().unwrap();
    fs::write(
        path.join("postgresql.auto.conf"),
        "listen_addresses = '*'\n",
    )
    .unwrap();
    assert!(c.start().is_err());
    fs::write(path.join("postgresql.auto.conf"), "# preserved\n").unwrap();
    fs::write(path.join("postmaster.pid"), b"1\n").unwrap();
    assert!(c.start().is_err());
    assert_eq!(fs::read(path.join("postmaster.pid")).unwrap(), b"1\n");
    fs::remove_file(path.join("postmaster.pid")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
    assert!(c.start().is_err());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let logs = c.logs().unwrap();
    assert!(!logs.text.contains(std::str::from_utf8(&secret.0).unwrap()));
    for (name, mode) in c.base.entries().unwrap() {
        if mode & libc::S_IFMT as u32 == libc::S_IFREG as u32 {
            let bytes = c.base.read(&name, 1024 * 1024).unwrap();
            assert!(
                !bytes.windows(secret.0.len()).any(|w| w == secret.0),
                "Plain credential found in retained metadata/log"
            );
        }
    }
    assert_eq!(c.cluster.as_ref().unwrap().id, id);
    assert!(listener.local_addr().is_ok());
    let layout = Layout::at(
        c.settings.join("home"),
        c.settings.clone(),
        c.source.parent().unwrap().to_owned(),
    );
    drop(c);
    let mut c = Controller::open(&layout, vault.clone()).unwrap();
    assert_eq!(c.cluster.as_ref().unwrap().id, id);
    assert!(!c.logs().unwrap().text.is_empty());
    c.start().unwrap();
    c.stop().unwrap();
    // A published cluster without registration is retained, never silently adopted.
    fs::rename(
        c.path.join("active.json"),
        c.path.join("receipt-retained.json"),
    )
    .unwrap();
    c.cluster = None;
    assert!(c.initialize(port).is_err());
    fs::rename(
        c.path.join("receipt-retained.json"),
        c.path.join("active.json"),
    )
    .unwrap();
    drop(c);
    let mut c = Controller::open(&layout, vault).unwrap();
    let rt = c.runtime().unwrap();
    let bytes = fs::read(rt.path.join("bin/pg_ctl")).unwrap();
    fs::set_permissions(
        rt.path.join("bin/pg_ctl"),
        fs::Permissions::from_mode(0o700),
    )
    .unwrap();
    fs::write(rt.path.join("bin/pg_ctl"), &bytes[..bytes.len() - 1]).unwrap();
    assert!(c.start().is_err());
}

#[test]
fn generic_data_removal_preserves_postgresql_directories_and_links() {
    let s = Scratch::new();
    assert!(crate::postgresql::ensure_data_removal_allowed(&s.0).is_ok());
    let d = Dir::absolute(&s.0).unwrap();
    let pg = d.make("postgresql").unwrap();
    pg.create("PG_VERSION", b"18\n", false).unwrap();
    assert!(crate::postgresql::ensure_data_removal_allowed(&s.0).is_err());
    assert_eq!(pg.read("PG_VERSION", 100).unwrap(), b"18\n");
    fs::rename(s.0.join("postgresql"), s.0.join("kept")).unwrap();
    symlink(s.0.join("kept"), s.0.join("postgresql")).unwrap();
    assert!(crate::postgresql::ensure_data_removal_allowed(&s.0).is_err());
}

#[test]
#[ignore = "Requires reviewed payload and query worker; creates disposable database fixtures with memory-only vault"]
fn stage3_native_catalog_rows_queries_types_cancellation_and_external_independence() {
    use crate::postgresql::{explorer::Explorer, explorer_contract as x};
    let (_scratch, mut c, vault) = fixture();
    c.source = std::env::var_os("FAIRWAY_TEST_POSTGRES_PAYLOAD")
        .expect("Provide reviewed payload")
        .into();
    runtime::provision(&c.base, &c.path, &c.source).unwrap();
    let port = TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    c.initialize(port).unwrap();
    c.start().unwrap();
    let id = c.cluster.as_ref().unwrap().id.clone();
    let layout = Layout::at(
        c.settings.join("home"),
        c.settings.clone(),
        c.source.parent().unwrap().into(),
    );
    let managed = Arc::new(Managed {
        layout: Some(layout),
        controller: Mutex::new(Some(c)),
        vault: vault.clone(),
    });
    let explorer = Arc::new(Explorer::default());
    let session = explorer
        .connect(x::Connect {
            confirmed: true,
            database: "postgres".into(),
            target: x::Target::Managed { cluster_id: id },
        })
        .unwrap();
    let call = |op| {
        explorer.execute(
            &managed,
            x::Request {
                job_id: format!("{:032x}", rand::random::<u128>()),
                session_id: session.id.clone(),
                operation: op,
            },
        )
    };
    let query = |sql: &str, parameters: Vec<Option<String>>, write: bool| {
        call(x::Operation::Query {
            sql: sql.into(),
            parameters,
            mode: if write {
                x::QueryMode::ReadWrite
            } else {
                x::QueryMode::ReadOnly
            },
            confirmed: true,
        })
    };
    let wait_for_sleep = |expected: &str| {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let result = match query(
                "SELECT count(*) FROM pg_catalog.pg_stat_activity \
                 WHERE application_name='fairway-postgresql-job' AND state='active' \
                 AND query='SELECT pg_sleep(30)' AND pid<>pg_catalog.pg_backend_pid()",
                vec![],
                false,
            ) {
                Err(error) if error.code == x::Code::Busy && Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(25));
                    continue;
                }
                result => result.unwrap(),
            };
            let actual = result.rows[0][0].as_deref();
            if actual == Some(expected) {
                return true;
            }
            if Instant::now() >= deadline {
                // Only fresh fixture sessions are queried; no passwords or SQL
                // text are included in these diagnostic fields.
                let activity=query("SELECT application_name,state,wait_event_type,wait_event FROM pg_catalog.pg_stat_activity WHERE backend_type='client backend' AND pid<>pg_catalog.pg_backend_pid()",vec![],false).unwrap();
                eprintln!("Fixture activity states: {:?}", activity.rows);
                return false;
            }
            std::thread::sleep(Duration::from_millis(25));
        }
    };
    query(
        "CREATE SCHEMA \"test\"\"; DROP SCHEMA nope;--\"",
        vec![],
        true,
    )
    .unwrap();
    let schema = "test\"; DROP SCHEMA nope;--";
    let relation = "rows\"; DROP TABLE nope;--";
    let target = "\"test\"\"; DROP SCHEMA nope;--\".\"rows\"\"; DROP TABLE nope;--\"";
    query(&format!("CREATE TABLE {target}(id bigint PRIMARY KEY,label text,document jsonb,token uuid,tags text[],stamp timestamptz,amount numeric(30,8),nothing text)"),vec![],true).unwrap();
    query(&format!("INSERT INTO {target} SELECT n,'日本 '||n,'{{\"big\":9007199254740993}}'::jsonb,'550e8400-e29b-41d4-a716-446655440000'::uuid,ARRAY['a,b','日本',NULL],TIMESTAMPTZ '2026-10-30 12:30:00+07',12345678901234567890.12345678,NULL FROM generate_series(1,205) n"),vec![],true).unwrap();
    query(
        &format!("CREATE VIEW public.stage3_view AS SELECT id,label FROM {target}"),
        vec![],
        true,
    )
    .unwrap();
    assert!(call(x::Operation::Databases)
        .unwrap()
        .rows
        .iter()
        .any(|r| r[1].as_deref() == Some("postgres")));
    assert!(call(x::Operation::Schemas)
        .unwrap()
        .rows
        .iter()
        .any(|r| r[1].as_deref() == Some(schema)));
    assert_eq!(
        call(x::Operation::Relations {
            schema: schema.into()
        })
        .unwrap()
        .rows[0][1]
            .as_deref(),
        Some(relation)
    );
    assert_eq!(
        call(x::Operation::Columns {
            schema: schema.into(),
            relation: relation.into()
        })
        .unwrap()
        .rows
        .len(),
        8
    );
    assert_eq!(
        call(x::Operation::Indexes {
            schema: schema.into(),
            relation: relation.into()
        })
        .unwrap()
        .rows
        .len(),
        1
    );
    assert!(!call(x::Operation::View {
        schema: "public".into(),
        relation: "stage3_view".into()
    })
    .unwrap()
    .rows
    .is_empty());
    let page = x::Page {
        limit: 2,
        offset: 1,
        sort: vec![x::Sort {
            column: "id".into(),
            direction: x::Direction::Desc,
        }],
        filters: vec![x::Filter {
            column: "id".into(),
            operator: x::FilterOperator::Gt,
            value: Some("200".into()),
        }],
    };
    let rows = call(x::Operation::Rows {
        schema: schema.into(),
        relation: relation.into(),
        page,
    })
    .unwrap();
    assert_eq!(rows.rows[0][0].as_deref(), Some("204"));
    assert_eq!(rows.has_more, Some(true));
    assert_eq!(
        rows.rows[0][2].as_deref(),
        Some("{\"big\": 9007199254740993}")
    );
    assert_eq!(
        rows.rows[0][3].as_deref(),
        Some("550e8400-e29b-41d4-a716-446655440000")
    );
    assert_eq!(rows.rows[0][4].as_deref(), Some("{\"a,b\",日本,NULL}"));
    assert_eq!(rows.rows[0][5].as_deref(), Some("2026-10-30 05:30:00+00"));
    assert_eq!(
        rows.rows[0][6].as_deref(),
        Some("12345678901234567890.12345678")
    );
    assert!(rows.rows[0][7].is_none());
    assert_eq!(rows.columns[2].type_oid, 3802);
    let q=query("SELECT $1::text AS duplicate, $2::text AS duplicate, NULL::uuid, ''::text, '\\x00ff'::bytea",vec![Some("'); DROP SCHEMA public CASCADE;--".into()),Some("exact".into())],false).unwrap();
    assert_eq!(q.columns[0].name, q.columns[1].name);
    assert!(q.rows[0][2].is_none());
    assert_eq!(q.rows[0][3].as_deref(), Some(""));
    assert_eq!(q.rows[0][4].as_deref(), Some("\\x00ff"));
    assert!(query("SELECT 1; SELECT 2", vec![], false).is_err());
    let denied = query(
        &format!("WITH d AS (DELETE FROM {target} RETURNING id) SELECT * FROM d"),
        vec![],
        false,
    )
    .unwrap_err();
    assert_eq!(denied.sql_state.as_deref(), Some("25006"));
    let limited = query("SELECT generate_series(1,1000000)", vec![], false).unwrap();
    assert!(limited.truncated);
    assert_eq!(limited.rows.len(), 200);
    assert_eq!(
        query("SELECT repeat('x',70000)", vec![], false)
            .unwrap_err()
            .code,
        x::Code::ResultLimit
    );
    let failed = query(
        "SELECT 'sensitive-diagnostic-marker'::integer",
        vec![],
        false,
    )
    .unwrap_err();
    assert!(!serde_json::to_string(&failed)
        .unwrap()
        .contains("sensitive-diagnostic-marker"));
    assert_eq!(failed.sql_state.as_deref(), Some("22P02"));
    // A write with oversized RETURNING data must never silently commit.
    assert!(query(&format!("DELETE FROM {target} RETURNING id"), vec![], true).is_err());
    assert_eq!(
        query(&format!("SELECT count(*) FROM {target}"), vec![], false)
            .unwrap()
            .rows[0][0]
            .as_deref(),
        Some("205")
    );
    let job = format!("{:032x}", rand::random::<u128>());
    let state = explorer.clone();
    let owner = managed.clone();
    let sid = session.id.clone();
    let jid = job.clone();
    let started = Instant::now();
    let thread = std::thread::spawn(move || {
        let dispatch_deadline = Instant::now() + Duration::from_secs(3);
        loop {
            let result = state.execute(
                &owner,
                x::Request {
                    job_id: jid.clone(),
                    session_id: sid.clone(),
                    operation: x::Operation::Query {
                        sql: "SELECT pg_sleep(30)".into(),
                        parameters: vec![],
                        mode: x::QueryMode::ReadOnly,
                        confirmed: true,
                    },
                },
            );
            // The observer can race the short metadata resolver. Only Busy is
            // retryable here: it means this job has not reached SQL dispatch.
            if result.as_ref().is_err_and(|e| e.code == x::Code::Busy)
                && Instant::now() < dispatch_deadline
            {
                std::thread::sleep(Duration::from_millis(25));
                continue;
            }
            return result;
        }
    });
    // Observe the actual server statement before requesting cancellation.
    if !wait_for_sleep("1") {
        let code = if thread.is_finished() {
            thread.join().unwrap().err().map(|e| e.code)
        } else {
            None
        };
        panic!("Sleep query was not active; completed job code: {code:?}");
    }
    let duplicate = explorer
        .execute(
            &managed,
            x::Request {
                job_id: job.clone(),
                session_id: session.id.clone(),
                operation: x::Operation::Schemas,
            },
        )
        .unwrap_err();
    assert_eq!(duplicate.code, x::Code::Busy);
    assert!(query("SELECT 42", vec![], false).is_ok());
    explorer.cancel(x::Cancel { job_id: job }).unwrap();
    assert_eq!(thread.join().unwrap().unwrap_err().code, x::Code::Cancelled);
    assert!(started.elapsed() < Duration::from_secs(8));
    assert!(wait_for_sleep("0"));
    assert_eq!(
        query("SELECT pg_sleep(30)", vec![], false)
            .unwrap_err()
            .code,
        x::Code::TimedOut
    );
    // A stalled external listener must time out without being stopped.
    let stalled = TcpListener::bind("127.0.0.1:0").unwrap();
    let stalled_session = explorer
        .connect(x::Connect {
            confirmed: true,
            database: "postgres".into(),
            target: x::Target::External {
                host: "127.0.0.1".into(),
                port: stalled.local_addr().unwrap().port(),
                user: "fairway".into(),
                password: x::Password::from_bytes(b"fixture-only").unwrap(),
                transport: x::Transport::Loopback,
            },
        })
        .unwrap();
    let started = Instant::now();
    let error = explorer
        .execute(
            &managed,
            x::Request {
                job_id: format!("{:032x}", rand::random::<u128>()),
                session_id: stalled_session.id.clone(),
                operation: x::Operation::Schemas,
            },
        )
        .unwrap_err();
    assert!(matches!(
        error.code,
        x::Code::TimedOut | x::Code::ConnectionFailed
    ));
    assert!(started.elapsed() < Duration::from_secs(7));
    assert!(stalled.local_addr().is_ok());
    explorer
        .disconnect(x::SessionRequest {
            session_id: stalled_session.id,
        })
        .unwrap();
    // Disconnect cancels that session's own running jobs too.
    let closing = explorer
        .connect(x::Connect {
            confirmed: true,
            database: "postgres".into(),
            target: x::Target::Managed {
                cluster_id: session_cluster_id(&managed),
            },
        })
        .unwrap();
    let state = explorer.clone();
    let owner = managed.clone();
    let sid = closing.id.clone();
    let thread = std::thread::spawn(move || {
        state.execute(
            &owner,
            x::Request {
                job_id: format!("{:032x}", rand::random::<u128>()),
                session_id: sid,
                operation: x::Operation::Query {
                    sql: "SELECT pg_sleep(30)".into(),
                    parameters: vec![],
                    mode: x::QueryMode::ReadOnly,
                    confirmed: true,
                },
            },
        )
    });
    std::thread::sleep(Duration::from_millis(600));
    explorer
        .disconnect(x::SessionRequest {
            session_id: closing.id,
        })
        .unwrap();
    assert_eq!(thread.join().unwrap().unwrap_err().code, x::Code::Cancelled);
    // Parent pipe EOF simulates owner loss; the worker cancels its own connection.
    {
        use std::io::Write;
        let mut e = managed
            .explorer_connection(Some(&session_cluster_id(&managed)), "postgres".into())
            .unwrap();
        e.operation = x::Operation::Query {
            sql: "SELECT pg_sleep(30)".into(),
            parameters: vec![],
            mode: x::QueryMode::ReadOnly,
            confirmed: true,
        };
        let mut input =
            crate::postgresql::explorer::wire::PrivateBytes(serde_json::to_vec(&e).unwrap());
        input.0.push(b'\n');
        let (read, mut write) = std::os::unix::net::UnixStream::pair().unwrap();
        let fd: std::os::fd::OwnedFd = read.into();
        let mut cmd = clean_command(std::env::var_os("FAIRWAY_TEST_POSTGRES_WORKER").unwrap());
        let mut child = cmd
            .arg("--fairway-postgres-query-worker")
            .stdin(Stdio::from(fd))
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        drop(cmd);
        write.write_all(&input.0).unwrap();
        drop(input);
        drop(e);
        assert!(wait_for_sleep("1"));
        drop(write);
        let until = Instant::now() + Duration::from_secs(4);
        loop {
            if let Some(status) = child.try_wait().unwrap() {
                assert!(status.success());
                break;
            }
            assert!(Instant::now() < until, "Owner-lost worker did not exit");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
    assert!(wait_for_sleep("0"));
    // A new unprivileged fixture role qualifies external access without granting
    // that target any lifecycle authority. Its verifier is copied only in memory.
    let verifier = query(
        "SELECT rolpassword FROM pg_catalog.pg_authid WHERE rolname='fairway'",
        vec![],
        false,
    )
    .unwrap()
    .rows[0][0]
        .clone()
        .unwrap();
    query(
        &format!(
            "CREATE ROLE stage3_read LOGIN PASSWORD '{}'",
            verifier.replace('\'', "''")
        ),
        vec![],
        true,
    )
    .unwrap();
    query(
        "GRANT USAGE ON SCHEMA \"test\"\"; DROP SCHEMA nope;--\" TO stage3_read",
        vec![],
        true,
    )
    .unwrap();
    query(
        &format!("GRANT SELECT ON {target} TO stage3_read"),
        vec![],
        true,
    )
    .unwrap();
    {
        let mut guard = managed.controller.lock().unwrap();
        let c = guard.as_mut().unwrap();
        let path = c.cluster_path(c.cluster.as_ref().unwrap()).unwrap();
        let dir = Dir::absolute(&path).unwrap();
        let mut hba = b"host all stage3_read 127.0.0.1/32 scram-sha-256\n".to_vec();
        hba.extend(dir.read("pg_hba.conf", 65536).unwrap());
        dir.atomic("pg_hba.conf", &hba).unwrap();
        c.cluster.as_mut().unwrap().hba_sha256 = runtime::hash(&hba);
        c.base
            .atomic("active.json", &json(c.cluster.as_ref().unwrap()).unwrap())
            .unwrap();
        unsafe {
            libc::kill(c.run.as_ref().unwrap().child.id() as i32, libc::SIGHUP);
        }
    }
    std::thread::sleep(Duration::from_millis(200));
    let password = {
        let v = vault.0.lock().unwrap();
        x::Password::from_bytes(&v.values().next().unwrap().0).unwrap()
    };
    let external = explorer
        .connect(x::Connect {
            confirmed: true,
            database: "postgres".into(),
            target: x::Target::External {
                host: "127.0.0.1".into(),
                port,
                user: "stage3_read".into(),
                password,
                transport: x::Transport::Loopback,
            },
        })
        .unwrap();
    let out = explorer
        .execute(
            &managed,
            x::Request {
                job_id: format!("{:032x}", rand::random::<u128>()),
                session_id: external.id.clone(),
                operation: x::Operation::Rows {
                    schema: schema.into(),
                    relation: relation.into(),
                    page: x::Page::default(),
                },
            },
        )
        .unwrap();
    assert_eq!(out.rows.len(), 50);
    explorer
        .disconnect(x::SessionRequest {
            session_id: external.id,
        })
        .unwrap();
    assert!(managed.snapshot().unwrap().running); // External disconnect never stops a server.
    explorer
        .disconnect(x::SessionRequest {
            session_id: session.id,
        })
        .unwrap();
    managed.stop(Confirmation { confirmed: true }).unwrap();
    {
        let guard = managed.controller.lock().unwrap();
        let c = guard.as_ref().unwrap();
        for (name, mode) in c.base.entries().unwrap() {
            if name.starts_with("log-") && mode & libc::S_IFMT as u32 == libc::S_IFREG as u32 {
                let data = c.base.read(&name, 1024 * 1024).unwrap();
                assert!(!data
                    .windows(b"sensitive-diagnostic-marker".len())
                    .any(|w| w == b"sensitive-diagnostic-marker"));
            }
        }
    }
    assert!(std::net::TcpStream::connect(("127.0.0.1", port)).is_err());
}

#[cfg(test)]
fn session_cluster_id(managed: &Managed) -> String {
    managed
        .controller
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .cluster
        .as_ref()
        .unwrap()
        .id
        .clone()
}

#[test]
fn explorer_resolution_preserves_busy_without_accessing_runtime_or_credentials() {
    use crate::postgresql::explorer_contract::Code;
    let managed = Managed::new(None);
    let _guard = managed.controller.lock().unwrap();
    let connection = managed
        .explorer_connection(None, "postgres".into())
        .err()
        .unwrap();
    assert_eq!(connection.code, Code::Busy);
    let lease = managed.explorer_lease().err().unwrap();
    assert_eq!(lease.code, Code::Busy);
}
