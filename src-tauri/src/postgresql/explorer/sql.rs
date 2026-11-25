//! PostgreSQL identifiers are quoted separately. All values use protocol parameters.
use super::super::explorer_contract::*;
pub fn identifier(s: &str) -> Result<String> {
    if s.is_empty() || s.len() > 63 || s.chars().any(char::is_control) {
        return Err(fail(
            Code::InvalidRequest,
            "PostgreSQL identifiers must contain 1–63 UTF-8 bytes without control characters",
        ));
    }
    Ok(format!("\"{}\"", s.replace('"', "\"\"")))
}
pub fn validate(op: &Operation) -> Result<()> {
    match op {
        Operation::Relations { schema } => {
            identifier(schema)?;
        }
        Operation::Columns { schema, relation }
        | Operation::Indexes { schema, relation }
        | Operation::View { schema, relation } => {
            identifier(schema)?;
            identifier(relation)?;
        }
        Operation::Rows {
            schema,
            relation,
            page,
        } => {
            identifier(schema)?;
            identifier(relation)?;
            if page.limit == 0
                || page.limit > 200
                || page.offset > 100_000
                || page.sort.len() > 3
                || page.filters.len() > 8
            {
                return Err(fail(Code::InvalidRequest, "Page limits exceeded"));
            }
            for sort in &page.sort {
                identifier(&sort.column)?;
            }
            for filter in &page.filters {
                identifier(&filter.column)?;
                if filter
                    .value
                    .as_ref()
                    .is_some_and(|s| s.len() > 4096 || s.contains('\0'))
                {
                    return Err(fail(Code::InvalidRequest, "Filter value exceeds bounds"));
                }
            }
        }
        Operation::Query {
            sql,
            parameters,
            mode,
            confirmed,
        } => {
            if !confirmed {
                return Err(fail(
                    Code::ConfirmationRequired,
                    "Explicit SQL execution authorization is required",
                ));
            }
            if sql.trim().is_empty()
                || sql.len() > 65536
                || sql.contains('\0')
                || parameters.len() > 32
                || parameters
                    .iter()
                    .flatten()
                    .any(|p| p.len() > 4096 || p.contains('\0'))
            {
                return Err(fail(
                    Code::InvalidRequest,
                    "SQL or parameter limits exceeded",
                ));
            }
            // This is a conservative surface gate, not a SQL parser/security sandbox.
            // The extended protocol and server read-only transaction enforce the boundary.
            if *mode == QueryMode::ReadOnly
                && !matches!(
                    first_word(sql).as_deref(),
                    Some("SELECT" | "WITH" | "VALUES" | "TABLE" | "SHOW" | "EXPLAIN")
                )
            {
                return Err(fail(
                    Code::ConfirmationRequired,
                    "This command requires explicitly authorized readWrite mode",
                ));
            }
        }
        _ => (),
    }
    Ok(())
}
fn first_word(mut s: &str) -> Option<String> {
    loop {
        s = s.trim_start();
        if let Some(rest) = s.strip_prefix("--") {
            s = rest.split_once('\n')?.1;
            continue;
        }
        if let Some(rest) = s.strip_prefix("/*") {
            let mut depth = 1;
            let mut i = 0;
            let b = rest.as_bytes();
            while i + 1 < b.len() {
                if &b[i..i + 2] == b"/*" {
                    depth += 1;
                    i += 2;
                } else if &b[i..i + 2] == b"*/" {
                    depth -= 1;
                    i += 2;
                    if depth == 0 {
                        break;
                    }
                } else {
                    i += 1;
                }
            }
            if depth != 0 {
                return None;
            }
            s = &rest[i..];
            continue;
        }
        let n = s.bytes().take_while(u8::is_ascii_alphabetic).count();
        return (n > 0).then(|| s[..n].to_ascii_uppercase());
    }
}
pub fn rows(
    schema: &str,
    relation: &str,
    page: &Page,
    columns: &[String],
    primary: &[String],
) -> Result<(String, Vec<Option<String>>, bool)> {
    let mut values = Vec::new();
    let mut clauses = Vec::new();
    let column = |s: &str| -> Result<String> {
        if !columns.iter().any(|c| c == s) {
            return Err(fail(
                Code::InvalidRequest,
                "Column is not available in this relation",
            ));
        }
        identifier(s)
    };
    for f in &page.filters {
        let c = column(&f.column)?;
        let clause = match f.operator {
            FilterOperator::IsNull => format!("{c} IS NULL"),
            FilterOperator::NotNull => format!("{c} IS NOT NULL"),
            _ => {
                let v = f
                    .value
                    .clone()
                    .ok_or_else(|| fail(Code::InvalidRequest, "This filter requires a value"))?;
                values.push(Some(v));
                let p = values.len();
                match f.operator {
                    FilterOperator::Contains => format!("pg_catalog.strpos({c}::text, ${p}) > 0"),
                    _ => {
                        let operator = match f.operator {
                            FilterOperator::Eq => "=",
                            FilterOperator::Ne => "<>",
                            FilterOperator::Lt => "<",
                            FilterOperator::Lte => "<=",
                            FilterOperator::Gt => ">",
                            FilterOperator::Gte => ">=",
                            _ => unreachable!(),
                        };
                        format!("{c} {operator} ${p}")
                    }
                }
            }
        };
        clauses.push(clause);
    }
    let mut order = Vec::new();
    let mut ordered = Vec::new();
    for sort in &page.sort {
        order.push(format!(
            "{} {} NULLS LAST",
            column(&sort.column)?,
            match sort.direction {
                Direction::Asc => "ASC",
                Direction::Desc => "DESC",
            }
        ));
        ordered.push(sort.column.clone());
    }
    for key in primary {
        if !ordered.contains(key) {
            order.push(format!("{} ASC", column(key)?));
        }
    }
    let selection = columns
        .iter()
        .map(|c| identifier(c))
        .collect::<Result<Vec<_>>>()?
        .join(",");
    if selection.is_empty() {
        return Err(fail(
            Code::InvalidRequest,
            "Relation has no browsable columns",
        ));
    }
    values.push(Some((page.limit + 1).to_string()));
    let limit = values.len();
    values.push(Some(page.offset.to_string()));
    let offset = values.len();
    Ok((
        format!(
            "SELECT {selection} FROM {}.{}{}{} LIMIT ${limit} OFFSET ${offset}",
            identifier(schema)?,
            identifier(relation)?,
            if clauses.is_empty() {
                String::new()
            } else {
                format!(" WHERE {}", clauses.join(" AND "))
            },
            if order.is_empty() {
                String::new()
            } else {
                format!(" ORDER BY {}", order.join(","))
            }
        ),
        values,
        !primary.is_empty(),
    ))
}
