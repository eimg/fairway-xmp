//! Read-only external-server check through the production session/worker path.
//! Credentials arrive on private stdin; no account values or credentials are logged.
use fairway_core::{
    config::Layout,
    postgresql::{
        explorer::Explorer, explorer_contract as x, managed_contract::Confirmation, Managed,
    },
};
use serde::Deserialize;
use std::{io::Read, path::PathBuf};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Input {
    connection: x::Connect,
    schema: String,
    relation: String,
}

fn main() {
    if fairway_core::postgresql::explorer::worker_entrypoint() {
        return;
    }
    if run().is_err() {
        eprintln!("External PostgreSQL check failed; credentials and database values withheld");
        std::process::exit(1);
    }
}

fn run() -> Result<(), ()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        eprintln!("Usage: postgresql_external_smoke RUNTIME_ROOT NEW_SCRATCH < private-input");
        return Err(());
    }
    let runtime = PathBuf::from(&args[0]).canonicalize().map_err(|_| ())?;
    let scratch = PathBuf::from(&args[1]);
    // Never adopt an existing profile or operate on a real managed cluster.
    std::fs::create_dir(&scratch).map_err(|_| ())?;
    let scratch = scratch.canonicalize().map_err(|_| ())?;
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(32769)
        .read_to_end(&mut bytes)
        .map_err(|_| ())?;
    let input: Result<Input, _> = serde_json::from_slice(&bytes);
    for b in &mut bytes {
        unsafe { std::ptr::write_volatile(b, 0) }
    }
    let input = input.map_err(|_| ())?;
    if !matches!(&input.connection.target, x::Target::External { .. }) {
        return Err(());
    }
    let managed = Managed::new(Some(Layout::at(
        scratch.join("home"),
        scratch.clone(),
        runtime,
    )));
    managed
        .provision(Confirmation { confirmed: true })
        .map_err(|_| ())?;
    let explorer = Explorer::default();
    let session = explorer.connect(input.connection).map_err(|_| ())?;
    let execute = |operation| {
        explorer
            .execute(
                &managed,
                x::Request {
                    session_id: session.id.clone(),
                    job_id: format!("{:032x}", rand::random::<u128>()),
                    operation,
                },
            )
            .map_err(|e| {
                eprintln!(
                    "Native check error: {:?}, SQLSTATE {:?}",
                    e.code, e.sql_state
                );
            })
    };
    let result = (|| {
        let identity = execute(x::Operation::Query {
            sql: "SELECT current_setting('server_version_num')".into(),
            parameters: vec![],
            mode: x::QueryMode::ReadOnly,
            confirmed: true,
        })?;
        let version = identity
            .rows
            .first()
            .and_then(|r| r.first())
            .and_then(|v| v.as_deref())
            .ok_or(())?;
        if !version.bytes().all(|b| b.is_ascii_digit()) {
            return Err(());
        }
        println!("Real native server_version_num: {version}");
        execute(x::Operation::Databases)?;
        let schemas = execute(x::Operation::Schemas)?;
        if !schemas
            .rows
            .iter()
            .any(|r| r.get(1).and_then(|v| v.as_deref()) == Some(input.schema.as_str()))
        {
            return Err(());
        }
        let relations = execute(x::Operation::Relations {
            schema: input.schema.clone(),
        })?;
        if !relations
            .rows
            .iter()
            .any(|r| r.get(1).and_then(|v| v.as_deref()) == Some(input.relation.as_str()))
        {
            return Err(());
        }
        for operation in [
            x::Operation::Columns {
                schema: input.schema.clone(),
                relation: input.relation.clone(),
            },
            x::Operation::Indexes {
                schema: input.schema.clone(),
                relation: input.relation.clone(),
            },
        ] {
            execute(operation)?;
        }
        let rows = execute(x::Operation::Rows {
            schema: input.schema.clone(),
            relation: input.relation.clone(),
            page: x::Page::default(),
        })?;
        println!("Real catalog/columns/indexes/rows succeeded: {} columns, {} visible rows, hasMore={}; values withheld", rows.columns.len(), rows.rows.len(), rows.has_more.unwrap_or(false));
        if rows.rows.is_empty() {
            return Err(());
        }
        // Exercise the GUI's parameterized filter/sort/page path without logging
        // the first account's identifier or changing any server data.
        let column = rows.columns.first().ok_or(())?.name.clone();
        let value = rows.rows[0].first().cloned().flatten().ok_or(())?;
        let filtered = execute(x::Operation::Rows {
            schema: input.schema.clone(),
            relation: input.relation.clone(),
            page: x::Page {
                limit: 1,
                offset: 0,
                sort: vec![x::Sort {
                    column: column.clone(),
                    direction: x::Direction::Desc,
                }],
                filters: vec![x::Filter {
                    column,
                    operator: x::FilterOperator::Eq,
                    value: Some(value),
                }],
            },
        })?;
        if filtered.rows.first() != rows.rows.first() {
            return Err(());
        }
        let next = execute(x::Operation::Rows {
            schema: input.schema,
            relation: input.relation,
            page: x::Page {
                limit: 1,
                offset: 1,
                ..x::Page::default()
            },
        })?;
        if rows.rows.len() > 1 && next.rows.first() != rows.rows.get(1) {
            return Err(());
        }
        println!("Real bound filter, descending sort and offset page succeeded; values withheld");
        Ok(())
    })();
    explorer
        .disconnect(x::SessionRequest {
            session_id: session.id,
        })
        .map_err(|_| ())?;
    result
}
