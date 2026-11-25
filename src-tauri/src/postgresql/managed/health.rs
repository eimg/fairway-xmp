//! Fixed loopback health only. No SQL editor, user SQL, inherited libpq config or password files.
use super::runtime::Runtime;
use super::secrets::Secret;
use super::{clean_command, failure, Cluster, Result};
use std::{
    ffi::{c_void, CString},
    path::Path,
};
pub fn responsiveness(rt: &Runtime, port: u16) -> Result<&'static str> {
    let mut command = clean_command(rt.path.join("bin/pg_isready"));
    command
        .args([
            "-h",
            "127.0.0.1",
            "-U",
            "fairway",
            "-d",
            "postgres",
            "-q",
            "-t",
            "1",
            "-p",
        ])
        .arg(port.to_string());
    match bounded_status(command, 3)?.code() {
        Some(0) => Ok("accepting"),
        Some(1) => Ok("rejecting"),
        Some(2) => Ok("noResponse"),
        _ => Err(failure("Pinned pg_isready check failed")),
    }
}

pub fn accepting(rt: &Runtime, port: u16) -> Result<bool> {
    Ok(responsiveness(rt, port)? == "accepting")
}
fn bounded_status(
    mut cmd: std::process::Command,
    seconds: u64,
) -> Result<std::process::ExitStatus> {
    let mut child = cmd
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| failure("Private PostgreSQL health process unavailable"))?;
    drop(cmd);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(seconds);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20))
            }
            _ => {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                };
                let _ = child.wait();
                return Err(failure("Private PostgreSQL health deadline exceeded"));
            }
        }
    }
}
#[derive(serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Request {
    runtime: std::path::PathBuf,
    cluster: Cluster,
    path: std::path::PathBuf,
    password: Vec<u8>,
}
impl Drop for Request {
    fn drop(&mut self) {
        for b in &mut self.password {
            unsafe { std::ptr::write_volatile(b, 0) };
        }
    }
}
/// Only the dedicated child loads libpq. Its environment was cleared before spawn.
pub fn worker_entrypoint() -> bool {
    if std::env::args_os().nth(1).as_deref()
        != Some(std::ffi::OsStr::new("--fairway-postgres-health-worker"))
    {
        return false;
    }
    use std::io::Read;
    let mut input = Secret(Vec::new());
    let result = (|| -> Result<bool> {
        std::io::stdin()
            .take(16385)
            .read_to_end(&mut input.0)
            .map_err(|_| failure("Private health input unavailable"))?;
        if input.0.len() > 16384 {
            return Err(failure("Private health input oversized"));
        }
        let r: Request = serde_json::from_slice(&input.0)
            .map_err(|_| failure("Private health input invalid"))?;
        if r.password.len() != 64
            || !r.password.iter().all(u8::is_ascii_hexdigit)
            || !super::valid_id(&r.cluster.id)
        {
            return Err(failure("Private health input invalid"));
        }
        let base = r
            .runtime
            .parent()
            .ok_or_else(|| failure("Private health runtime invalid"))?;
        let rt = super::runtime::installed(&super::Dir::absolute(base)?, base)?;
        if rt.path != r.runtime || rt.manifest_sha256 != r.cluster.runtime_sha256 {
            return Err(failure("Private health runtime changed"));
        }
        authenticate_direct(&rt, &r.cluster, &r.path, &Secret(r.password.clone()))
    })();
    // No raw libpq diagnostics, server messages or credentials are ever emitted.
    drop(input);
    std::process::exit(match result {
        Ok(true) => 0,
        Ok(false) => 1,
        Err(_) => 2,
    });
}
pub fn authenticate(rt: &Runtime, cluster: &Cluster, path: &Path, secret: &Secret) -> Result<bool> {
    use std::io::Write;
    let executable =
        std::env::current_exe().map_err(|_| failure("Private health worker unavailable"))?;
    #[cfg(test)]
    let executable = std::env::var_os("FAIRWAY_TEST_POSTGRES_WORKER")
        .map(std::path::PathBuf::from)
        .unwrap_or(executable);
    let input = Secret(
        serde_json::to_vec(&Request {
            runtime: rt.path.clone(),
            cluster: cluster.clone(),
            path: path.to_owned(),
            password: secret.0.clone(),
        })
        .map_err(|_| failure("Private health request unavailable"))?,
    );
    let (read, mut write) = std::os::unix::net::UnixStream::pair()
        .map_err(|_| failure("Private health pipe unavailable"))?;
    let fd: std::os::fd::OwnedFd = read.into();
    write
        .write_all(&input.0)
        .map_err(|_| failure("Private health input unavailable"))?;
    write
        .shutdown(std::net::Shutdown::Write)
        .map_err(|_| failure("Private health input unavailable"))?;
    let mut cmd = clean_command(executable);
    let mut child = cmd
        .arg("--fairway-postgres-health-worker")
        .stdin(std::process::Stdio::from(fd))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|_| failure("Private health worker unavailable"))?;
    drop(cmd);
    drop(write);
    drop(input);
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(8);
    loop {
        match child.try_wait() {
            Ok(Some(status)) => {
                return match status.code() {
                    Some(0) => Ok(true),
                    Some(1) => Ok(false),
                    _ => Err(failure("Private health verification failed")),
                }
            }
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(20))
            }
            _ => {
                unsafe {
                    libc::kill(-(child.id() as i32), libc::SIGKILL);
                };
                let _ = child.wait();
                return Err(failure("Private health worker deadline exceeded"));
            }
        }
    }
}

struct Library(*mut c_void);
impl Drop for Library {
    fn drop(&mut self) {
        unsafe { libc::dlclose(self.0) };
    }
}
impl Library {
    unsafe fn symbol<T: Copy>(&self, name: &std::ffi::CStr) -> Result<T> {
        let p = unsafe { libc::dlsym(self.0, name.as_ptr()) };
        if p.is_null() {
            Err(failure("Pinned libpq health API unavailable"))
        } else {
            Ok(unsafe { std::mem::transmute_copy(&p) })
        }
    }
}
fn authenticate_direct(
    rt: &Runtime,
    cluster: &Cluster,
    path: &Path,
    secret: &Secret,
) -> Result<bool> {
    let library = CString::new(
        rt.path
            .join("lib/libpq.5.dylib")
            .to_str()
            .ok_or_else(|| failure("Invalid pinned libpq path"))?,
    )
    .map_err(|_| failure("Invalid pinned libpq path"))?;
    let handle = unsafe { libc::dlopen(library.as_ptr(), libc::RTLD_NOW | libc::RTLD_LOCAL) };
    if handle.is_null() {
        return Err(failure("Cannot load verified libpq health library"));
    }
    let lib = Library(handle);
    unsafe {
        let connect: unsafe extern "C" fn(*const *const i8, *const *const i8, i32) -> *mut c_void =
            lib.symbol(c"PQconnectdbParams")?;
        let status: unsafe extern "C" fn(*const c_void) -> i32 = lib.symbol(c"PQstatus")?;
        let finish: unsafe extern "C" fn(*mut c_void) = lib.symbol(c"PQfinish")?;
        let exec: unsafe extern "C" fn(*mut c_void, *const i8) -> *mut c_void =
            lib.symbol(c"PQexec")?;
        let result_status: unsafe extern "C" fn(*const c_void) -> i32 =
            lib.symbol(c"PQresultStatus")?;
        let rows: unsafe extern "C" fn(*const c_void) -> i32 = lib.symbol(c"PQntuples")?;
        let value: unsafe extern "C" fn(*const c_void, i32, i32) -> *const i8 =
            lib.symbol(c"PQgetvalue")?;
        let clear: unsafe extern "C" fn(*mut c_void) = lib.symbol(c"PQclear")?;
        let keys = [
            c"host",
            c"hostaddr",
            c"port",
            c"user",
            c"dbname",
            c"password",
            c"passfile",
            c"sslmode",
            c"gssencmode",
            c"connect_timeout",
            c"options",
            c"application_name",
        ];
        let port = CString::new(cluster.port.to_string()).unwrap();
        let password = CString::new(secret.0.clone())
            .map_err(|_| failure("Secure credential format invalid"))?;
        let values = [
            c"127.0.0.1",
            c"127.0.0.1",
            &port,
            c"fairway",
            c"postgres",
            &password,
            c"/dev/null",
            c"disable",
            c"disable",
            c"2",
            c"-c statement_timeout=2000 -c lock_timeout=1000",
            c"fairway-managed-health",
        ];
        let mut k: Vec<_> = keys.iter().map(|s| s.as_ptr()).collect();
        k.push(std::ptr::null());
        let mut v: Vec<_> = values.iter().map(|s| s.as_ptr()).collect();
        v.push(std::ptr::null());
        let connection = connect(k.as_ptr(), v.as_ptr(), 0);
        // Wipe the temporary C-string copy after libpq has consumed it.
        let mut copy = password.into_bytes_with_nul();
        for b in &mut copy {
            std::ptr::write_volatile(b, 0);
        }
        if connection.is_null() {
            return Ok(false);
        }
        struct Connection(*mut c_void, unsafe extern "C" fn(*mut c_void));
        impl Drop for Connection {
            fn drop(&mut self) {
                unsafe { (self.1)(self.0) }
            }
        }
        let conn = Connection(connection, finish);
        if status(conn.0) != 0 {
            return Ok(false);
        }
        let result=exec(conn.0,c"SELECT current_database(), current_user, current_setting('server_version'), current_setting('data_directory'), current_setting('cluster_name')".as_ptr());
        if result.is_null() {
            return Ok(false);
        }
        struct Query(*mut c_void, unsafe extern "C" fn(*mut c_void));
        impl Drop for Query {
            fn drop(&mut self) {
                unsafe { (self.1)(self.0) }
            }
        }
        let result = Query(result, clear);
        if result_status(result.0) != 2 || rows(result.0) != 1 {
            return Ok(false);
        }
        let expected = [
            "postgres".to_string(),
            "fairway".into(),
            "18.6".into(),
            path.to_str()
                .ok_or_else(|| failure("Invalid managed cluster path"))?
                .into(),
            format!("fairway-{}", cluster.id),
        ];
        for (column, expected) in expected.iter().enumerate() {
            let p = value(result.0, 0, column as i32);
            if p.is_null() {
                return Ok(false);
            }
            let actual = std::ffi::CStr::from_ptr(p).to_string_lossy();
            if column == 2 {
                if actual.split_ascii_whitespace().next() != Some(expected.as_str()) {
                    return Ok(false);
                }
            } else if actual != *expected {
                return Ok(false);
            }
        }
        Ok(true)
    }
}
