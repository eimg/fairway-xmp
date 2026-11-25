//! Private clean-environment libpq worker. No renderer paths or server control.
use super::super::explorer_contract::*;
use super::{catalog, sql, wire::Envelope};
use std::{
    ffi::{c_void, CStr, CString},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
type Ptr = *mut c_void;
macro_rules! api {($($field:ident:$symbol:literal($($arg:ty),*)->$ret:ty),* $(,)?)=>{
struct Api{library:Ptr,$($field:unsafe extern "C" fn($($arg),*)->$ret),*}
impl Api {fn load(path:&std::path::Path)->Result<Self>{
    let name=CString::new(path.to_str().ok_or_else(||fail(Code::Unavailable,"Pinned client unavailable"))?).map_err(|_|fail(Code::Unavailable,"Pinned client unavailable"))?;
    let library=unsafe{libc::dlopen(name.as_ptr(),libc::RTLD_NOW|libc::RTLD_LOCAL)};
    if library.is_null(){return Err(fail(Code::Unavailable,"Pinned client unavailable"));}
    unsafe fn get<T:Copy>(library:Ptr,name:&CStr)->Result<T>{let p=unsafe{libc::dlsym(library,name.as_ptr())};if p.is_null(){Err(fail(Code::Unavailable,"Pinned client API unavailable"))}else{Ok(unsafe{std::mem::transmute_copy(&p)})}}
    let result=(||{Ok(Self{library,$($field:unsafe{get(library,CStr::from_bytes_with_nul(concat!($symbol,"\0").as_bytes()).unwrap())?}),*})})();
    if result.is_err(){unsafe{libc::dlclose(library);}}
    result
}}
impl Drop for Api {fn drop(&mut self){unsafe{libc::dlclose(self.library);}}}
}}
api! {
    connect:"PQconnectStartParams"(*const *const i8,*const *const i8,i32)->Ptr,
    connect_poll:"PQconnectPoll"(Ptr)->i32, status:"PQstatus"(Ptr)->i32, finish:"PQfinish"(Ptr)->(),
    nonblocking:"PQsetnonblocking"(Ptr,i32)->i32, socket:"PQsocket"(Ptr)->i32,
    send:"PQsendQueryParams"(Ptr,*const i8,i32,*const u32,*const *const i8,*const i32,*const i32,i32)->i32,
    single:"PQsetSingleRowMode"(Ptr)->i32, flush:"PQflush"(Ptr)->i32, consume:"PQconsumeInput"(Ptr)->i32,
    busy:"PQisBusy"(Ptr)->i32, get:"PQgetResult"(Ptr)->Ptr, result_status:"PQresultStatus"(Ptr)->i32,
    fields:"PQnfields"(Ptr)->i32,name:"PQfname"(Ptr,i32)->*const i8, type_oid:"PQftype"(Ptr,i32)->u32,
    modifier:"PQfmod"(Ptr,i32)->i32, table:"PQftable"(Ptr,i32)->u32, attribute:"PQftablecol"(Ptr,i32)->i32,
    null:"PQgetisnull"(Ptr,i32,i32)->i32, length:"PQgetlength"(Ptr,i32,i32)->i32,value:"PQgetvalue"(Ptr,i32,i32)->*const i8,
    tag:"PQcmdStatus"(Ptr)->*const i8, affected:"PQcmdTuples"(Ptr)->*const i8, clear:"PQclear"(Ptr)->(),
    error_field:"PQresultErrorField"(Ptr,i32)->*const i8, transaction:"PQtransactionStatus"(Ptr)->i32,
    notice:"PQsetNoticeProcessor"(Ptr,Option<unsafe extern "C" fn(Ptr,*const i8)>,Ptr)->Option<unsafe extern "C" fn(Ptr,*const i8)>,
    cancel_create:"PQcancelCreate"(Ptr)->Ptr,cancel_start:"PQcancelStart"(Ptr)->i32,cancel_poll:"PQcancelPoll"(Ptr)->i32,
    cancel_socket:"PQcancelSocket"(Ptr)->i32,cancel_finish:"PQcancelFinish"(Ptr)->()
}
struct Connection {
    ptr: Ptr,
    api: Api,
    cancel: Arc<AtomicBool>,
    deadline: Instant,
    bytes: usize,
}
impl Drop for Connection {
    fn drop(&mut self) {
        unsafe { (self.api.finish)(self.ptr) }
    }
}
struct Row<'a> {
    ptr: Ptr,
    api: &'a Api,
}
impl Drop for Row<'_> {
    fn drop(&mut self) {
        unsafe { (self.api.clear)(self.ptr) }
    }
}
unsafe extern "C" fn silent_notice(_: Ptr, _: *const i8) {}
fn text(p: *const i8) -> String {
    if p.is_null() {
        String::new()
    } else {
        unsafe { CStr::from_ptr(p) }.to_string_lossy().into_owned()
    }
}
fn poll(fd: i32, write: bool) {
    if fd >= 0 {
        let mut p = libc::pollfd {
            fd,
            events: if write { libc::POLLOUT } else { libc::POLLIN },
            revents: 0,
        };
        unsafe {
            libc::poll(&mut p, 1, 20);
        }
    } else {
        std::thread::sleep(Duration::from_millis(10));
    }
}
impl Connection {
    fn cancelled(&self) -> Result<()> {
        let code = if self.cancel.load(Ordering::SeqCst) {
            Some(Code::Cancelled)
        } else if Instant::now() > self.deadline {
            Some(Code::TimedOut)
        } else {
            None
        };
        if let Some(code) = code {
            self.cancel_query();
            return Err(fail(
                code,
                "PostgreSQL operation cancelled or deadline exceeded",
            ));
        }
        Ok(())
    }
    fn cancel_query(&self) {
        unsafe {
            let c = (self.api.cancel_create)(self.ptr);
            if c.is_null() {
                return;
            }
            if (self.api.cancel_start)(c) == 1 {
                let until = Instant::now() + Duration::from_secs(1);
                loop {
                    let status = (self.api.cancel_poll)(c);
                    if status == 0 || status == 3 || Instant::now() >= until {
                        break;
                    }
                    poll((self.api.cancel_socket)(c), status == 2);
                }
            }
            (self.api.cancel_finish)(c);
        }
    }
    fn connect(e: &Envelope, cancel: Arc<AtomicBool>) -> Result<Self> {
        let api = Api::load(&e.runtime.join("lib/libpq.5.dylib"))?;
        // Passwords go only through private stdin/memory. libpq never expands a conninfo dbname.
        let keys = [
            "host",
            "hostaddr",
            "port",
            "user",
            "dbname",
            "password",
            "passfile",
            "sslmode",
            "gssencmode",
            "connect_timeout",
            "client_encoding",
            "application_name",
            "options",
        ];
        let base_options="-c statement_timeout=10000 -c lock_timeout=1000 -c idle_in_transaction_session_timeout=10000 -c default_transaction_read_only=on -c search_path=pg_catalog -c timezone=UTC -c datestyle=ISO,YMD -c intervalstyle=iso_8601 -c extra_float_digits=3 -c bytea_output=hex";
        let options = if e.managed.is_some() {
            format!("{base_options} -c log_statement=none -c log_min_error_statement=panic -c log_min_messages=panic -c log_parameter_max_length=0 -c log_parameter_max_length_on_error=0")
        } else {
            base_options.to_owned()
        };
        let mut values: Vec<CString> = [
            e.host.clone(),
            e.host.clone(),
            e.port.to_string(),
            e.user.clone(),
            e.database.clone(),
            String::new(),
            "/dev/null".into(),
            "disable".into(),
            "disable".into(),
            "3".into(),
            "UTF8".into(),
            "fairway-postgresql-job".into(),
            options,
        ]
        .into_iter()
        .map(|s| {
            CString::new(s).map_err(|_| fail(Code::InvalidRequest, "Invalid connection value"))
        })
        .collect::<Result<_>>()?;
        values[5] = CString::new(e.password.bytes())
            .map_err(|_| fail(Code::InvalidRequest, "Invalid credential"))?;
        let keys: Vec<CString> = keys.iter().map(|s| CString::new(*s).unwrap()).collect();
        let mut kp: Vec<_> = keys.iter().map(|s| s.as_ptr()).collect();
        kp.push(std::ptr::null());
        let mut vp: Vec<_> = values.iter().map(|s| s.as_ptr()).collect();
        vp.push(std::ptr::null());
        let ptr = unsafe { (api.connect)(kp.as_ptr(), vp.as_ptr(), 0) };
        let mut password = values.swap_remove(5).into_bytes_with_nul();
        for b in &mut password {
            unsafe { std::ptr::write_volatile(b, 0) }
        }
        if ptr.is_null() {
            return Err(fail(Code::ConnectionFailed, "PostgreSQL connection failed"));
        }
        let c = Self {
            ptr,
            api,
            cancel,
            deadline: Instant::now() + Duration::from_secs(3),
            bytes: 0,
        };
        unsafe {
            (c.api.notice)(ptr, Some(silent_notice), std::ptr::null_mut());
        }
        loop {
            c.cancelled()?;
            let status = unsafe { (c.api.connect_poll)(ptr) };
            match status {
                3 => break,
                0 => return Err(fail(Code::ConnectionFailed, "PostgreSQL connection failed")),
                _ => poll(unsafe { (c.api.socket)(ptr) }, status == 2),
            }
        }
        if unsafe { (c.api.status)(ptr) } != 0 || unsafe { (c.api.nonblocking)(ptr, 1) } != 0 {
            return Err(fail(Code::ConnectionFailed, "PostgreSQL connection failed"));
        }
        Ok(c)
    }
    fn query(
        &mut self,
        sql: &str,
        parameters: &[Option<String>],
        row_limit: usize,
    ) -> Result<Output> {
        self.cancelled()?;
        let sql = CString::new(sql).map_err(|_| fail(Code::InvalidRequest, "Invalid SQL"))?;
        let params: Vec<_> = parameters
            .iter()
            .map(|v| {
                v.as_ref()
                    .map(|s| {
                        CString::new(s.as_str())
                            .map_err(|_| fail(Code::InvalidRequest, "Invalid parameter"))
                    })
                    .transpose()
            })
            .collect::<Result<_>>()?;
        let pointers: Vec<_> = params
            .iter()
            .map(|p| p.as_ref().map_or(std::ptr::null(), |p| p.as_ptr()))
            .collect();
        if unsafe {
            (self.api.send)(
                self.ptr,
                sql.as_ptr(),
                pointers.len() as i32,
                std::ptr::null(),
                pointers.as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                0,
            )
        } != 1
            || unsafe { (self.api.single)(self.ptr) } != 1
        {
            return Err(fail(
                Code::QueryFailed,
                "PostgreSQL command could not be dispatched",
            ));
        }
        let mut out = Output {
            columns: vec![],
            rows: vec![],
            command: String::new(),
            affected_rows: None,
            truncated: false,
            has_more: None,
            warnings: vec![],
            outcome: Outcome::NotCommitted,
        };
        loop {
            self.cancelled()?;
            match unsafe { (self.api.flush)(self.ptr) } {
                0 => (),
                1 => {
                    poll(unsafe { (self.api.socket)(self.ptr) }, true);
                    continue;
                }
                _ => return Err(fail(Code::ConnectionFailed, "PostgreSQL transport failed")),
            }
            if unsafe { (self.api.consume)(self.ptr) } != 1 {
                return Err(fail(Code::ConnectionFailed, "PostgreSQL transport failed"));
            }
            if unsafe { (self.api.busy)(self.ptr) } != 0 {
                poll(unsafe { (self.api.socket)(self.ptr) }, false);
                continue;
            }
            let ptr = unsafe { (self.api.get)(self.ptr) };
            if ptr.is_null() {
                return Ok(out);
            }
            let result = Row {
                ptr,
                api: &self.api,
            };
            let status = unsafe { (self.api.result_status)(ptr) };
            if ![1, 2, 9].contains(&status) {
                let mut error=fail(Code::QueryFailed,"PostgreSQL rejected the command; diagnostic text is withheld to protect credentials and data");
                let state = text(unsafe { (self.api.error_field)(ptr, 'C' as i32) });
                if state.len() == 5
                    && state
                        .bytes()
                        .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit())
                {
                    if state == "57014" {
                        let primary = text(unsafe { (self.api.error_field)(ptr, 'M' as i32) });
                        error.code = if primary.ends_with("statement timeout") {
                            Code::TimedOut
                        } else {
                            Code::Cancelled
                        };
                    }
                    error.sql_state = Some(state);
                }
                error.position = text(unsafe { (self.api.error_field)(ptr, 'P' as i32) })
                    .parse()
                    .ok();
                return Err(error);
            }
            let fields = unsafe { (self.api.fields)(ptr) };
            if !(0..=128).contains(&fields) {
                self.cancel_query();
                return Err(fail(Code::ResultLimit, "Column limit exceeded"));
            }
            if out.columns.is_empty() {
                for i in 0..fields {
                    let oid = unsafe { (self.api.type_oid)(ptr, i) };
                    let name = text(unsafe { (self.api.name)(ptr, i) });
                    if name.len() > 256 {
                        return Err(fail(Code::ResultLimit, "Column metadata limit exceeded"));
                    }
                    out.columns.push(Column {
                        name,
                        type_oid: oid,
                        type_name: type_name(oid),
                        type_modifier: unsafe { (self.api.modifier)(ptr, i) },
                        table_oid: unsafe { (self.api.table)(ptr, i) },
                        attribute_number: unsafe { (self.api.attribute)(ptr, i) },
                    });
                }
            }
            if status == 9 {
                if out.rows.len() >= row_limit {
                    drop(result);
                    self.cancel_query();
                    out.truncated = true;
                    out.warnings.push(
                        "Result limit reached; statement cancelled and connection closed".into(),
                    );
                    return Ok(out);
                }
                let mut row = Vec::new();
                for i in 0..fields {
                    if unsafe { (self.api.null)(ptr, 0, i) } != 0 {
                        row.push(None);
                        continue;
                    }
                    let len = unsafe { (self.api.length)(ptr, 0, i) };
                    if !(0..=65536).contains(&len) {
                        self.cancel_query();
                        return Err(fail(Code::ResultLimit, "Cell limit exceeded"));
                    }
                    self.bytes += len as usize;
                    if self.bytes > 256 * 1024 {
                        self.cancel_query();
                        return Err(fail(Code::ResultLimit, "Result byte limit exceeded"));
                    }
                    let b = unsafe {
                        std::slice::from_raw_parts(
                            (self.api.value)(ptr, 0, i).cast::<u8>(),
                            len as usize,
                        )
                    };
                    row.push(Some(
                        std::str::from_utf8(b)
                            .map_err(|_| fail(Code::QueryFailed, "Invalid result encoding"))?
                            .to_owned(),
                    ));
                }
                out.rows.push(row);
            } else {
                out.command = text(unsafe { (self.api.tag)(ptr) });
                let count = text(unsafe { (self.api.affected)(ptr) });
                if !count.is_empty() && count.bytes().all(|b| b.is_ascii_digit()) {
                    out.affected_rows = Some(count);
                }
            }
        }
    }
}
fn type_name(oid: u32) -> String {
    match oid {
        16 => "bool",
        17 => "bytea",
        20 => "int8",
        21 => "int2",
        23 => "int4",
        25 => "text",
        114 => "json",
        700 => "float4",
        701 => "float8",
        1043 => "varchar",
        1082 => "date",
        1083 => "time",
        1114 => "timestamp",
        1184 => "timestamptz",
        1186 => "interval",
        1700 => "numeric",
        2950 => "uuid",
        3802 => "jsonb",
        1007 => "int4[]",
        1009 => "text[]",
        1016 => "int8[]",
        2951 => "uuid[]",
        3807 => "jsonb[]",
        _ => return format!("oid:{oid}"),
    }
    .into()
}
pub fn execute(e: &Envelope, cancel: Arc<AtomicBool>) -> Result<Output> {
    sql::validate(&e.operation)?;
    let mut c = Connection::connect(e, cancel)?;
    c.deadline = Instant::now() + Duration::from_secs(12);
    let write = matches!(
        &e.operation,
        Operation::Query {
            mode: QueryMode::ReadWrite,
            ..
        }
    );
    let result = (|| -> Result<Output> {
        c.query(
            if write {
                "BEGIN READ WRITE"
            } else {
                "BEGIN READ ONLY"
            },
            &[],
            1,
        )?;
        let identity=c.query(if e.managed.is_some(){"SELECT current_database(),current_user,current_setting('server_version_num'),current_setting('cluster_name'),current_setting('data_directory')"}else{"SELECT current_database(),current_user,current_setting('server_version_num')"},&[],1)?;
        // External non-superusers cannot necessarily inspect data_directory.
        let r = identity
            .rows
            .first()
            .ok_or_else(|| fail(Code::ConnectionFailed, "Database identity unavailable"))?;
        if r[0].as_deref() != Some(e.database.as_str()) || r[1].as_deref() != Some(e.user.as_str())
        {
            return Err(fail(Code::ConnectionFailed, "Database identity mismatch"));
        }
        if !supported_server(r[2].as_deref(), e.managed.is_some()) {
            return Err(fail(
                Code::Unavailable,
                "Supported servers: external PostgreSQL 17.x or 18.6; managed PostgreSQL 18.6",
            ));
        }
        if let Some(expected) = &e.managed {
            if r[3].as_deref() != Some(expected.cluster_name.as_str())
                || r[4].as_deref() != expected.data_directory.to_str()
            {
                return Err(fail(
                    Code::ConnectionFailed,
                    "Managed database identity mismatch",
                ));
            }
        }
        let mut out = match &e.operation {
            Operation::Query {
                sql, parameters, ..
            } => c.query(sql, parameters, 200)?,
            Operation::Rows {
                schema,
                relation,
                page,
            } => {
                let args = vec![Some(schema.clone()), Some(relation.clone())];
                let columns = c.query(catalog::COLUMNS, &args, 200)?;
                if columns.truncated || columns.rows.len() > 128 {
                    return Err(fail(Code::ResultLimit, "Relation column limit exceeded"));
                }
                let names: Vec<_> = columns.rows.iter().filter_map(|r| r[0].clone()).collect();
                let keys = c.query(catalog::PRIMARY, &args, 128)?;
                let keys: Vec<_> = keys.rows.iter().filter_map(|r| r[0].clone()).collect();
                let (sql, params, stable) = sql::rows(schema, relation, page, &names, &keys)?;
                let mut out = c.query(&sql, &params, 201)?;
                let more = out.rows.len() > page.limit as usize;
                out.rows.truncate(page.limit as usize);
                out.has_more = Some(more);
                out.warnings
                    .push("Offset pages can change under concurrent writes".into());
                if !stable {
                    out.warnings
                        .push("No primary key: page ordering is not guaranteed stable".into());
                }
                out
            }
            other => {
                let (sql, args) = catalog::query(other)
                    .ok_or_else(|| fail(Code::InvalidRequest, "Unsupported operation"))?;
                c.query(sql, &args, 200)?
            }
        };
        if out.truncated {
            if write {
                return Err(fail(
                    Code::ResultLimit,
                    "Write result exceeded limits; commit was not attempted",
                ));
            }
            return Ok(out);
        }
        if unsafe { (c.api.transaction)(c.ptr) } != 2 {
            let mut err = fail(
                Code::QueryFailed,
                "Transaction control and commands escaping the job transaction are unsupported",
            );
            err.outcome = Outcome::Unknown;
            return Err(err);
        }
        if write {
            if let Err(mut err) = c.query("COMMIT", &[], 1) {
                err.outcome = Outcome::Unknown;
                return Err(err);
            }
            out.outcome = Outcome::Committed;
        } else {
            c.query("ROLLBACK", &[], 1)?;
        }
        Ok(out)
    })();
    result.map_err(|mut e| {
        if e.outcome == Outcome::NotStarted {
            e.outcome = Outcome::NotCommitted;
        }
        e
    })
}

// External catalog operations use the PostgreSQL 17 catalog surface. The client
// payload and managed cluster identity remain independently pinned to 18.6.
fn supported_server(version: Option<&str>, managed: bool) -> bool {
    let Some(version) = version else { return false };
    if version == "180006" {
        return true;
    }
    !managed
        && version.len() == 6
        && version.bytes().all(|b| b.is_ascii_digit())
        && version
            .parse::<u32>()
            .is_ok_and(|v| (170000..180000).contains(&v))
}

#[cfg(test)]
mod version_tests {
    use super::supported_server;

    #[test]
    fn external_postgresql_17_does_not_relax_managed_or_unknown_versions() {
        for version in ["170000", "170012", "179999", "180006"] {
            assert!(supported_server(Some(version), false));
            assert_eq!(supported_server(Some(version), true), version == "180006");
        }
        for version in [
            None,
            Some("160012"),
            Some("180007"),
            Some("190000"),
            Some("17.12"),
            Some("+170012"),
            Some("0170012"),
        ] {
            assert!(!supported_server(version, false));
            assert!(!supported_server(version, true));
        }
    }
}
