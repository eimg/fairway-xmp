use super::*;
#[test]
fn identifiers_values_and_page_limits_have_separate_boundaries() {
    assert_eq!(
        sql::identifier("a\"; DROP TABLE x;--").unwrap(),
        "\"a\"\"; DROP TABLE x;--\""
    );
    assert!(sql::identifier(&"日".repeat(22)).is_err());
    let op: Operation = serde_json::from_str(
        r#"{"kind":"rows","schema":"public","relation":"x","page":{"limit":201}}"#,
    )
    .unwrap();
    assert!(sql::validate(&op).is_err());
    let page = Page {
        limit: 2,
        offset: 0,
        sort: vec![Sort {
            column: "id".into(),
            direction: Direction::Desc,
        }],
        filters: vec![Filter {
            column: "name".into(),
            operator: FilterOperator::Contains,
            value: Some("');DROP TABLE x;--".into()),
        }],
    };
    let (query, params, stable) = sql::rows(
        "weird\"schema",
        "table",
        &page,
        &["id".into(), "name".into()],
        &["id".into()],
    )
    .unwrap();
    assert!(!query.contains("DROP"));
    assert_eq!(params[0].as_deref(), Some("');DROP TABLE x;--"));
    assert!(query.contains("\"weird\"\"schema\".\"table\""));
    assert!(stable);
}
#[test]
fn execution_is_explicit_and_read_only_by_default() {
    let mut query: Operation = serde_json::from_str(
        r#"{"kind":"query","sql":"/* outer /* inner */ ok */ SELECT $1","parameters":["exact"]}"#,
    )
    .unwrap();
    assert_eq!(
        sql::validate(&query).unwrap_err().code,
        Code::ConfirmationRequired
    );
    if let Operation::Query { confirmed, .. } = &mut query {
        *confirmed = true;
    }
    assert!(sql::validate(&query).is_ok());
    for sql in [
        "COMMIT",
        "SET transaction_read_only=off",
        "-- comment\nINSERT INTO x VALUES (1)",
    ] {
        assert_eq!(
            sql::validate(&Operation::Query {
                sql: sql.into(),
                parameters: vec![],
                mode: QueryMode::ReadOnly,
                confirmed: true
            })
            .unwrap_err()
            .code,
            Code::ConfirmationRequired
        );
    }
    assert!(serde_json::from_str::<Request>(
        r#"{"jobId":"x","sessionId":"s","operation":{"kind":"schemas"},"path":"/personal"}"#
    )
    .is_err());
}
#[test]
fn external_sessions_are_ephemeral_loopback_only_and_do_not_claim_authentication() {
    let state = Explorer::default();
    let r:Connect=serde_json::from_str(r#"{"confirmed":true,"database":"postgres","target":{"kind":"external","host":"example.com","port":5432,"user":"me","password":"secret","transport":"verifyFull"}}"#).unwrap();
    assert_eq!(state.connect(r).unwrap_err().code, Code::Unavailable);
    let r:Connect=serde_json::from_str(r#"{"confirmed":true,"database":"postgres","target":{"kind":"external","host":"127.0.0.1","port":5432,"user":"me","password":"secret","transport":"loopback"}}"#).unwrap();
    let view = state.connect(r).unwrap();
    let s = serde_json::to_string(&view).unwrap();
    assert!(!s.contains("secret"));
    assert!(s.contains("memoryOnly"));
    state
        .disconnect(SessionRequest {
            session_id: view.id,
        })
        .unwrap();
    assert!(state.sessions.lock().unwrap().is_empty());
}
#[test]
fn pre_dispatch_cancellation_is_not_lost_and_closed_sessions_stay_stale() {
    let state = Explorer::default();
    let managed = Managed::new(None);
    let job = "a".repeat(32);
    state
        .cancel(Cancel {
            job_id: job.clone(),
        })
        .unwrap();
    let r = Request {
        job_id: job,
        session_id: "b".repeat(32),
        operation: Operation::Schemas,
    };
    assert_eq!(
        state.execute(&managed, r).unwrap_err().code,
        Code::Cancelled
    );
    assert!(state.jobs.lock().unwrap().is_empty());
    assert_eq!(
        state
            .execute(
                &managed,
                Request {
                    job_id: "c".repeat(32),
                    session_id: "b".repeat(32),
                    operation: Operation::Schemas
                }
            )
            .unwrap_err()
            .code,
        Code::StaleSession
    );
}
