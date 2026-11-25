//! Independent sessions/jobs; neither an external profile nor a job owns a server.
use super::{explorer_contract::*, Managed};
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod catalog;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod native;
#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
mod process;
mod sql;
#[cfg(test)]
mod tests;
pub(crate) mod wire;
struct Connection {
    database: String,
    target: ConnectionTarget,
}
enum ConnectionTarget {
    Managed(String),
    External {
        host: String,
        port: u16,
        user: String,
        password: Password,
    },
}
#[derive(Default)]
pub struct Explorer {
    sessions: Mutex<HashMap<String, Connection>>,
    jobs: Mutex<HashMap<String, Arc<AtomicBool>>>,
    pending_cancels: Mutex<HashMap<String, std::time::Instant>>,
}
fn id(s: &str) -> bool {
    s.len() == 32 && s.bytes().all(|b| b.is_ascii_hexdigit())
}
fn database(s: &str) -> Result<()> {
    sql::identifier(s).map(|_| ())
}
impl Explorer {
    pub fn connect(&self, r: Connect) -> Result<Session> {
        if !r.confirmed {
            return Err(fail(
                Code::ConfirmationRequired,
                "Authorize this database connection",
            ));
        }
        database(&r.database)?;
        let (target, kind) = match r.target {
            Target::Managed { cluster_id } => {
                if !id(&cluster_id) {
                    return Err(fail(
                        Code::InvalidRequest,
                        "Invalid managed cluster identity",
                    ));
                }
                (ConnectionTarget::Managed(cluster_id), "managed")
            }
            Target::External {
                host,
                port,
                user,
                password,
                transport,
            } => {
                if !matches!(transport, Transport::Loopback) {
                    return Err(fail(Code::Unavailable,"Remote verify-full TLS requires a qualified TLS-capable client; no downgrade is allowed"));
                }
                if !["127.0.0.1", "::1"].contains(&host.as_str())
                    || port == 0
                    || password.bytes().len() > 4096
                    || password.bytes().contains(&0)
                {
                    return Err(fail(
                        Code::InvalidRequest,
                        "External loopback connection parameters are invalid",
                    ));
                }
                sql::identifier(&user)?;
                (
                    ConnectionTarget::External {
                        host,
                        port,
                        user,
                        password,
                    },
                    "external",
                )
            }
        };
        let mut sessions = self
            .sessions
            .lock()
            .map_err(|_| fail(Code::Unavailable, "Session state unavailable"))?;
        if sessions.len() >= 16 {
            return Err(fail(
                Code::Busy,
                "Close a database session before opening another",
            ));
        }
        let id = format!("{:032x}", rand::random::<u128>());
        let view = Session {
            id: id.clone(),
            kind: kind.into(),
            database: r.database.clone(),
            persistence: "memoryOnly",
            authentication: "notChecked",
        };
        sessions.insert(
            id,
            Connection {
                database: r.database,
                target,
            },
        );
        Ok(view)
    }
    pub fn disconnect(&self, r: SessionRequest) -> Result<()> {
        self.sessions
            .lock()
            .map_err(|_| fail(Code::Unavailable, "Session state unavailable"))?
            .remove(&r.session_id);
        // Jobs own their already-resolved credentials until completion; explicitly
        // cancel all jobs for this session via the registry association below.
        self.cancel_session(&r.session_id);
        Ok(())
    }
    fn cancel_session(&self, session: &str) {
        if let Ok(jobs) = self.jobs.lock() {
            for (id, flag) in jobs.iter() {
                if id.starts_with(&format!("{session}:")) {
                    flag.store(true, Ordering::SeqCst);
                }
            }
        }
    }
    pub fn cancel(&self, r: Cancel) -> Result<()> {
        if !id(&r.job_id) {
            return Err(fail(Code::InvalidRequest, "Invalid job identity"));
        }
        let jobs = self
            .jobs
            .lock()
            .map_err(|_| fail(Code::Unavailable, "Job registry unavailable"))?;
        let mut found = false;
        for (key, flag) in jobs.iter() {
            if key.ends_with(&format!(":{}", r.job_id)) {
                flag.store(true, Ordering::SeqCst);
                found = true;
            }
        }
        if !found {
            let mut pending = self
                .pending_cancels
                .lock()
                .map_err(|_| fail(Code::Unavailable, "Job registry unavailable"))?;
            pending.retain(|_, t| t.elapsed() < std::time::Duration::from_secs(30));
            if pending.len() >= 64 {
                return Err(fail(Code::Busy, "Pending cancellation limit reached"));
            }
            pending.insert(r.job_id, std::time::Instant::now());
        }
        Ok(())
    }
    pub fn cancel_all(&self) {
        if let Ok(jobs) = self.jobs.lock() {
            for f in jobs.values() {
                f.store(true, Ordering::SeqCst);
            }
        }
    }
    pub fn execute(&self, managed: &Managed, r: Request) -> Result<Output> {
        sql::validate(&r.operation)?;
        if !id(&r.job_id) || !id(&r.session_id) {
            return Err(fail(
                Code::InvalidRequest,
                "Invalid job or session identity",
            ));
        }
        let key = format!("{}:{}", r.session_id, r.job_id);
        let flag = Arc::new(AtomicBool::new(false));
        // Resolve credentials only after a unique job slot has been reserved.
        {
            let mut jobs = self
                .jobs
                .lock()
                .map_err(|_| fail(Code::Unavailable, "Job registry unavailable"))?;
            if jobs.len() >= 2 || jobs.keys().any(|k| k.ends_with(&format!(":{}", r.job_id))) {
                return Err(fail(
                    Code::Busy,
                    "PostgreSQL job slots are occupied or identity is already in use",
                ));
            }
            let mut pending = self
                .pending_cancels
                .lock()
                .map_err(|_| fail(Code::Unavailable, "Job registry unavailable"))?;
            if pending
                .remove(&r.job_id)
                .is_some_and(|t| t.elapsed() < std::time::Duration::from_secs(30))
            {
                return Err(fail(Code::Cancelled, "Job cancelled before dispatch"));
            }
            jobs.insert(key.clone(), flag.clone());
        }
        struct Guard<'a>(&'a Explorer, String);
        impl Drop for Guard<'_> {
            fn drop(&mut self) {
                if let Ok(mut j) = self.0.jobs.lock() {
                    j.remove(&self.1);
                }
            }
        }
        let _guard = Guard(self, key);
        let mut e = {
            let sessions = self
                .sessions
                .lock()
                .map_err(|_| fail(Code::Unavailable, "Session state unavailable"))?;
            let c = sessions
                .get(&r.session_id)
                .ok_or_else(|| fail(Code::StaleSession, "Database session is closed or stale"))?;
            match &c.target {
                ConnectionTarget::Managed(id) => {
                    managed.explorer_connection(Some(id), c.database.clone())?
                }
                ConnectionTarget::External {
                    host,
                    port,
                    user,
                    password,
                } => {
                    let mut e = managed.explorer_connection(None, c.database.clone())?;
                    e.host = host.clone();
                    e.port = *port;
                    e.user = user.clone();
                    e.password = password.copy();
                    e
                }
            }
        };
        e.operation = r.operation;
        validate_connection(&e)?;
        #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
        {
            let _lease = managed.explorer_lease()?;
            process::run(e, flag)
        }
        #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
        {
            let _ = (e, flag);
            Err(fail(
                Code::Unavailable,
                "PostgreSQL client is not qualified on this platform",
            ))
        }
    }
}
pub(crate) fn validate_connection(e: &wire::Envelope) -> Result<()> {
    database(&e.database)?;
    sql::identifier(&e.user)?;
    if !["127.0.0.1", "::1"].contains(&e.host.as_str())
        || e.port == 0
        || e.password.bytes().len() > 4096
        || e.password.bytes().contains(&0)
    {
        return Err(fail(
            Code::InvalidRequest,
            "Invalid private connection request",
        ));
    }
    sql::validate(&e.operation)
}
impl Drop for Explorer {
    fn drop(&mut self) {
        self.cancel_all();
    }
}
pub fn worker_entrypoint() -> bool {
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    {
        wire::entrypoint()
    }
    #[cfg(not(all(target_os = "macos", target_arch = "aarch64")))]
    {
        false
    }
}
