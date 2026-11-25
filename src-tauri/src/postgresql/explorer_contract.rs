//! Renderer contracts contain no server lifecycle authority. Passwords are ephemeral input only.
use serde::{Deserialize, Serialize};
#[derive(Deserialize)]
#[serde(transparent)]
pub struct Password(String);
impl Password {
    pub(crate) fn bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }
    pub(crate) fn copy(&self) -> Self {
        Self(self.0.clone())
    }
    #[cfg(all(target_os = "macos", target_arch = "aarch64"))]
    pub(crate) fn from_bytes(b: &[u8]) -> Result<Self> {
        Ok(Self(
            std::str::from_utf8(b)
                .map_err(|_| fail(Code::Unavailable, "Credential format unavailable"))?
                .to_owned(),
        ))
    }
}
impl Drop for Password {
    fn drop(&mut self) {
        unsafe {
            for b in self.0.as_bytes_mut() {
                std::ptr::write_volatile(b, 0)
            }
        }
    }
}
#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Target {
    #[serde(rename_all = "camelCase")]
    Managed { cluster_id: String },
    External {
        host: String,
        port: u16,
        user: String,
        password: Password,
        transport: Transport,
    },
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Transport {
    Loopback,
    VerifyFull,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Connect {
    pub confirmed: bool,
    pub database: String,
    pub target: Target,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Session {
    pub id: String,
    pub kind: String,
    pub database: String,
    pub persistence: &'static str,
    pub authentication: &'static str,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SessionRequest {
    pub session_id: String,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase", deny_unknown_fields)]
pub enum Operation {
    Databases,
    Schemas,
    Relations {
        schema: String,
    },
    Columns {
        schema: String,
        relation: String,
    },
    Indexes {
        schema: String,
        relation: String,
    },
    View {
        schema: String,
        relation: String,
    },
    Rows {
        schema: String,
        relation: String,
        #[serde(default)]
        page: Page,
    },
    Query {
        sql: String,
        #[serde(default)]
        parameters: Vec<Option<String>>,
        #[serde(default)]
        mode: QueryMode,
        #[serde(default)]
        confirmed: bool,
    },
}
#[derive(Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum QueryMode {
    #[default]
    ReadOnly,
    ReadWrite,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Page {
    #[serde(default = "page_size")]
    pub limit: u16,
    #[serde(default)]
    pub offset: u32,
    #[serde(default)]
    pub sort: Vec<Sort>,
    #[serde(default)]
    pub filters: Vec<Filter>,
}
fn page_size() -> u16 {
    50
}
impl Default for Page {
    fn default() -> Self {
        Self {
            limit: 50,
            offset: 0,
            sort: vec![],
            filters: vec![],
        }
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Sort {
    pub column: String,
    pub direction: Direction,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Direction {
    Asc,
    Desc,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Filter {
    pub column: String,
    pub operator: FilterOperator,
    pub value: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum FilterOperator {
    Eq,
    Ne,
    Lt,
    Lte,
    Gt,
    Gte,
    Contains,
    IsNull,
    NotNull,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub job_id: String,
    pub session_id: String,
    pub operation: Operation,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Cancel {
    pub job_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Column {
    pub name: String,
    pub type_oid: u32,
    pub type_name: String,
    pub type_modifier: i32,
    pub table_oid: u32,
    pub attribute_number: i32,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Output {
    pub columns: Vec<Column>,
    pub rows: Vec<Vec<Option<String>>>,
    pub command: String,
    pub affected_rows: Option<String>,
    pub truncated: bool,
    pub has_more: Option<bool>,
    pub warnings: Vec<String>,
    pub outcome: Outcome,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Outcome {
    NotStarted,
    NotCommitted,
    Committed,
    Unknown,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum Code {
    InvalidRequest,
    ConfirmationRequired,
    Unavailable,
    ConnectionFailed,
    Cancelled,
    TimedOut,
    QueryFailed,
    ResultLimit,
    Busy,
    StaleSession,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Error {
    pub code: Code,
    pub message: String,
    pub sql_state: Option<String>,
    pub position: Option<u32>,
    pub outcome: Outcome,
}
pub type Result<T> = std::result::Result<T, Error>;
pub(crate) fn fail(code: Code, message: &str) -> Error {
    Error {
        code,
        message: message.into(),
        sql_state: None,
        position: None,
        outcome: Outcome::NotStarted,
    }
}
