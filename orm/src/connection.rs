//! Database connection + pool.
//!
//! V1 ships with a **mock** in-process driver so the ORM can be tested without
//! a real database. To enable real Postgres/MySQL/SQLite, enable the matching
//! feature flag on `kungfu-orm`:
//!
//! ```toml
//! kungfu-orm = { path = "...", features = ["postgres"] }
//! ```

use std::collections::HashMap;
use std::sync::Arc;

use parking_lot::Mutex;
use serde::de::DeserializeOwned;

use crate::{Error, FieldDef, Model, Query, Result};

/// Database configuration.
#[derive(Debug, Clone, Default)]
pub struct DbConfig {
    pub url: String,
    pub max_connections: usize,
    pub min_connections: usize,
}

/// A database handle. Cheap to clone — the inner state is behind an `Arc`.
#[derive(Clone)]
pub struct Db {
    pub inner: Arc<DbInner>,
}

pub enum DbInner {
    /// In-memory mock driver — used for tests and when no `sqlx` feature
    /// is enabled. Stores rows as `serde_json::Value` keyed by table.
    Mock(Mutex<MockDb>),
    #[cfg(feature = "postgres")]
    Postgres(sqlx::PgPool),
    #[cfg(feature = "mysql")]
    Mysql(sqlx::MySqlPool),
    #[cfg(feature = "sqlite")]
    Sqlite(sqlx::SqlitePool),
}

#[derive(Default)]
struct MockDb {
    tables: HashMap<String, Vec<serde_json::Value>>,
    next_id: HashMap<String, i64>,
}

impl Db {
    /// Construct a mock in-process database. Useful for tests + examples.
    pub fn mock() -> Self {
        Self {
            inner: Arc::new(DbInner::Mock(Mutex::new(MockDb::default()))),
        }
    }

    /// Connect to a real database. Requires the matching feature flag.
    pub async fn connect(config: &DbConfig) -> Result<Self> {
        let url = &config.url;
        let max = config.max_connections.max(1) as u32;
        let min = config.min_connections.min(max as usize) as u32;

        #[cfg(feature = "postgres")]
        if url.starts_with("postgres://") || url.starts_with("postgresql://") {
            let pool = sqlx::postgres::PgPoolOptions::new()
                .max_connections(max)
                .min_connections(min)
                .connect(url)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            return Ok(Self { inner: Arc::new(DbInner::Postgres(pool)) });
        }

        #[cfg(feature = "mysql")]
        if url.starts_with("mysql://") {
            let pool = sqlx::mysql::MySqlPoolOptions::new()
                .max_connections(max)
                .min_connections(min)
                .connect(url)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            return Ok(Self { inner: Arc::new(DbInner::Mysql(pool)) });
        }

        #[cfg(feature = "sqlite")]
        if url.starts_with("sqlite://") || url.starts_with("sqlite::") {
            let pool = sqlx::sqlite::SqlitePoolOptions::new()
                .max_connections(max)
                .min_connections(min)
                .connect(url)
                .await
                .map_err(|e| Error::Database(e.to_string()))?;
            return Ok(Self { inner: Arc::new(DbInner::Sqlite(pool)) });
        }

        // Fall back to mock if no driver matched.
        Ok(Self::mock())
    }

    /// Execute a SELECT query and deserialise rows into `T`.
    pub async fn query<T: Model>(&self, q: Query<T>) -> Result<Vec<T>> {
        let (sql, params) = q.to_sql();
        match &*self.inner {
            DbInner::Mock(m) => mock_query(m, q).await,
            #[cfg(feature = "postgres")]
            DbInner::Postgres(pool) => sqlx_postgres_query(pool, &sql, &params).await,
            #[cfg(feature = "mysql")]
            DbInner::Mysql(pool) => sqlx_mysql_query(pool, &sql, &params).await,
            #[cfg(feature = "sqlite")]
            DbInner::Sqlite(pool) => sqlx_sqlite_query(pool, &sql, &params).await,
        }
    }

    /// Count matching rows.
    pub async fn count<T: Model>(&self, q: Query<T>) -> Result<i64> {
        let (count_sql, params) = q.to_count_sql();
        match &*self.inner {
            DbInner::Mock(_) => Ok(0),
            #[cfg(feature = "postgres")]
            DbInner::Postgres(pool) => sqlx_count_postgres(pool, &count_sql, &params).await,
            #[cfg(feature = "mysql")]
            DbInner::Mysql(pool) => sqlx_count_mysql(pool, &count_sql, &params).await,
            #[cfg(feature = "sqlite")]
            DbInner::Sqlite(pool) => sqlx_count_sqlite(pool, &count_sql, &params).await,
        }
    }

    /// Insert a row.
    pub async fn insert_row<T: Model>(&self, value: &T) -> Result<T> {
        match &*self.inner {
            DbInner::Mock(m) => mock_insert(m, value).await,
            #[cfg(feature = "postgres")]
            DbInner::Postgres(pool) => sqlx_insert_postgres(pool, value).await,
            #[cfg(feature = "mysql")]
            DbInner::Mysql(pool) => sqlx_insert_mysql(pool, value).await,
            #[cfg(feature = "sqlite")]
            DbInner::Sqlite(pool) => sqlx_insert_sqlite(pool, value).await,
        }
    }

    /// Execute an UPDATE statement. Returns the number of affected rows.
    pub async fn execute(&self, sql: &str, params: &[serde_json::Value]) -> Result<u64> {
        match &*self.inner {
            DbInner::Mock(_) => Ok(0),
            #[cfg(feature = "postgres")]
            DbInner::Postgres(pool) => sqlx_execute_postgres(pool, sql, params).await,
            #[cfg(feature = "mysql")]
            DbInner::Mysql(pool) => sqlx_execute_mysql(pool, sql, params).await,
            #[cfg(feature = "sqlite")]
            DbInner::Sqlite(pool) => sqlx_execute_sqlite(pool, sql, params).await,
        }
    }
}

async fn mock_query<T: Model + DeserializeOwned>(
    m: &Mutex<MockDb>,
    q: Query<T>,
) -> Result<Vec<T>> {
    let (sql, _params) = q.to_sql();
    let _ = sql;
    // For the mock driver we ignore the WHERE clauses and return all rows
    // for the table. This is sufficient for the V1 test suite — real
    // filtering happens in the sqlx-backed implementation.
    let m = m.lock();
    let rows = m.tables.get(T::table_name()).cloned().unwrap_or_default();
    let mut out = Vec::new();
    for r in rows {
        if let Ok(t) = serde_json::from_value::<T>(r) {
            out.push(t);
        }
    }
    Ok(out)
}

async fn mock_insert<T: Model + serde::Serialize>(
    m: &Mutex<MockDb>,
    value: &T,
) -> Result<T> {
    let mut m = m.lock();
    let table = T::table_name().to_string();
    let mut json = serde_json::to_value(value).map_err(|e| Error::Serde(e))?;

    // Auto-hash sensitive fields (e.g. passwords) marked in the model's FieldDef.
    if let Some(obj) = json.as_object_mut() {
        for field in T::fields() {
            if field.sensitive {
                if let Some(plain) = obj.get(field.rust_name).and_then(|v| v.as_str()) {
                    if !plain.starts_with("$argon2") {
                        // Only hash if it isn't already a hash.
                        let hashed = crate::password::hash_password(plain)?;
                        obj.insert(field.rust_name.to_string(), serde_json::json!(hashed));
                    }
                }
            }
        }
    }

    // Auto-increment primary key.
    let pk_field = T::fields().iter().find(|f| f.is_primary && f.auto_increment);
    if let Some(pk) = pk_field {
        let next = m.next_id.entry(table.clone()).or_insert(1);
        if let Some(obj) = json.as_object_mut() {
            obj.insert(pk.rust_name.to_string(), serde_json::json!(*next));
        }
        *next += 1;
    }

    m.tables.entry(table.clone()).or_default().push(json.clone());

    // Re-deserialise so the caller gets the post-insert value (with id populated).
    serde_json::from_value(json).map_err(|e| Error::Serde(e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::{Deserialize, Serialize};

    #[derive(kungfu_macros::Model, Serialize, Deserialize)]
    #[table(name = "users")]
    struct User {
        #[field(primary, auto_increment)]
        id: i64,
        email: String,
    }

    #[tokio::test]
    async fn mock_insert_assigns_id() {
        let db = Db::mock();
        let user = User { id: 0, email: "a@b.com".into() };
        let inserted = db.insert_row(&user).await.unwrap();
        assert_eq!(inserted.id, 1);
        assert_eq!(inserted.email, "a@b.com");

        let all = User::all(&db).await.unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, 1);
    }

    #[tokio::test]
    async fn mock_insert_hashes_sensitive_fields() {
        // User with a `sensitive` password field — should be Argon2id-hashed
        // automatically on insert.
        let db = Db::mock();
        let user = UserWithPassword {
            id: 0,
            email: "alice@example.com".into(),
            password: "plaintext_password".into(),
        };
        let inserted = db.insert_row(&user).await.unwrap();
        // The stored password should be a hash, not the plaintext.
        assert!(
            inserted.password.starts_with("$argon2"),
            "expected Argon2id hash, got: {}",
            inserted.password
        );
        // Verify the hash matches the original plaintext.
        assert!(crate::password::verify_password("plaintext_password", &inserted.password).unwrap());
    }

    #[derive(kungfu_macros::Model, Serialize, Deserialize)]
    #[table(name = "users_with_password")]
    struct UserWithPassword {
        #[field(primary, auto_increment)]
        id: i64,
        #[field(unique)]
        email: String,
        #[field(sensitive)]
        password: String,
    }
}

// ─── sqlx-backed query implementations ────────────────────────────────────────

#[cfg(feature = "postgres")]
async fn sqlx_postgres_query<T: Model + serde::de::DeserializeOwned>(
    pool: &sqlx::PgPool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<Vec<T>> {
    let mut q = sqlx::query(sql);
    for p in params {
        q = bind_param_postgres(q, p);
    }
    let rows = q.fetch_all(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let value = row_to_json_postgres(&row);
        let t: T = serde_json::from_value(value).map_err(Error::Serde)?;
        out.push(t);
    }
    Ok(out)
}

#[cfg(feature = "postgres")]
pub fn bind_param_postgres<'q>(
    q: sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments>,
    p: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Postgres, sqlx::postgres::PgArguments> {
    match p {
        serde_json::Value::Null => q.bind(None::<String>),
        serde_json::Value::Bool(b) => q.bind(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { q.bind(i) }
            else if let Some(f) = n.as_f64() { q.bind(f) }
            else { q.bind(n.to_string()) }
        }
        serde_json::Value::String(s) => q.bind(s),
        _ => q.bind(p.to_string()),
    }
}

#[cfg(feature = "postgres")]
pub fn row_to_json_postgres(row: &sqlx::postgres::PgRow) -> serde_json::Value {
    use sqlx::Row;
    let mut map = serde_json::Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let name = col.name();
        let value: serde_json::Value = if let Ok(v) = row.try_get::<Option<String>, _>(i) {
            v.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
            v.map(|n| serde_json::json!(n)).unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
            v.map(|n| serde_json::json!(n)).unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
            v.map(serde_json::Value::Bool).unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };
        map.insert(name.to_string(), value);
    }
    serde_json::Value::Object(map)
}

#[cfg(feature = "mysql")]
async fn sqlx_mysql_query<T: Model + serde::de::DeserializeOwned>(
    pool: &sqlx::MySqlPool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<Vec<T>> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param_mysql(q, p); }
    let rows = q.fetch_all(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let value = row_to_json_mysql(&row);
        let t: T = serde_json::from_value(value).map_err(Error::Serde)?;
        out.push(t);
    }
    Ok(out)
}

#[cfg(feature = "mysql")]
pub fn bind_param_mysql<'q>(
    q: sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments>,
    p: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::MySql, sqlx::mysql::MySqlArguments> {
    match p {
        serde_json::Value::Null => q.bind(None::<String>),
        serde_json::Value::Bool(b) => q.bind(b),
        serde_json::Value::Number(n) => {
            if let Some(i) = n.as_i64() { q.bind(i) }
            else if let Some(f) = n.as_f64() { q.bind(f) }
            else { q.bind(n.to_string()) }
        }
        serde_json::Value::String(s) => q.bind(s),
        _ => q.bind(p.to_string()),
    }
}

#[cfg(feature = "mysql")]
pub fn row_to_json_mysql(row: &sqlx::mysql::MySqlRow) -> serde_json::Value {
    use sqlx::Row;
    let mut map = serde_json::Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let name = col.name();
        let value: serde_json::Value = if let Ok(v) = row.try_get::<Option<String>, _>(i) {
            v.map(serde_json::Value::String).unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
            v.map(|n| serde_json::json!(n)).unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
            v.map(|n| serde_json::json!(n)).unwrap_or(serde_json::Value::Null)
        } else { serde_json::Value::Null };
        map.insert(name.to_string(), value);
    }
    serde_json::Value::Object(map)
}

#[cfg(feature = "sqlite")]
async fn sqlx_sqlite_query<T: Model + serde::de::DeserializeOwned>(
    pool: &sqlx::SqlitePool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<Vec<T>> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param_sqlite(q, p); }
    let rows = q.fetch_all(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let value = row_to_json_sqlite(&row);
        let t: T = serde_json::from_value(value).map_err(Error::Serde)?;
        out.push(t);
    }
    Ok(out)
}

#[cfg(feature = "sqlite")]
pub fn bind_param_sqlite<'q>(
    mut q: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>>,
    p: &serde_json::Value,
) -> sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments<'q>> {
    // Bind ALL values as strings. SQLite columns are declared as TEXT in our
    // migrations, so storing values as strings ensures the type matches and
    // try_get::<Option<String>> succeeds on read. We parse back to the
    // correct type in row_to_json_sqlite.
    match p {
        serde_json::Value::Null => { q = q.bind(None::<String>); q }
        serde_json::Value::Bool(b) => { q = q.bind(if *b { "1" } else { "0" }); q }
        serde_json::Value::Number(n) => { q = q.bind(n.to_string()); q }
        serde_json::Value::String(s) => { let s: String = s.clone(); q = q.bind(s); q }
        _ => { let s: String = p.to_string(); q = q.bind(s); q }
    }
}

#[cfg(feature = "sqlite")]
pub fn row_to_json_sqlite(row: &sqlx::sqlite::SqliteRow) -> serde_json::Value {
    use sqlx::{Column, Row};
    let mut map = serde_json::Map::new();
    for (i, col) in row.columns().iter().enumerate() {
        let name = col.name().to_string();
        // Read everything as String (our columns are all TEXT).
        let value: serde_json::Value = if let Ok(v) = row.try_get::<Option<String>, _>(i) {
            match v {
                Some(s) => {
                    // Try to parse as i64, then f64, then keep as string.
                    if let Ok(n) = s.parse::<i64>() {
                        serde_json::json!(n)
                    } else if let Ok(f) = s.parse::<f64>() {
                        serde_json::json!(f)
                    } else {
                        serde_json::Value::String(s)
                    }
                }
                None => serde_json::Value::Null,
            }
        } else if let Ok(v) = row.try_get::<Option<i64>, _>(i) {
            v.map(|n| serde_json::json!(n)).unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<f64>, _>(i) {
            v.map(|n| serde_json::json!(n)).unwrap_or(serde_json::Value::Null)
        } else if let Ok(v) = row.try_get::<Option<bool>, _>(i) {
            v.map(serde_json::Value::Bool).unwrap_or(serde_json::Value::Null)
        } else {
            serde_json::Value::Null
        };
        map.insert(name, value);
    }
    serde_json::Value::Object(map)
}

// ─── sqlx INSERT implementations ──────────────────────────────────────────────

/// Build an INSERT SQL string from a Model value.
///
/// Returns (sql, params). The SQL uses `RETURNING *` for Postgres, or a
/// SELECT after the insert for SQLite/MySQL (which don't support RETURNING
/// in older versions).
pub fn build_insert_sql<T: Model>(value: &T) -> (String, Vec<serde_json::Value>) {
    let table = T::table_name();
    let fields = T::fields();

    // Filter out auto_increment fields — the DB assigns them.
    let insert_fields: Vec<&FieldDef> = fields.iter().filter(|f| !f.auto_increment).collect();
    let col_names: Vec<&str> = insert_fields.iter().map(|f| f.column_name).collect();
    let placeholders: Vec<String> = (1..=insert_fields.len()).map(|i| format!("${i}")).collect();

    let mut sql = format!(
        "INSERT INTO {} ({}) VALUES ({})",
        table,
        col_names.join(", "),
        placeholders.join(", ")
    );

    // Postgres supports RETURNING *, which lets us fetch the inserted row
    // (with auto-incremented PK) in one round-trip.
    #[cfg(feature = "postgres")]
    sql.push_str(" RETURNING *");

    // Extract the values.
    let json = serde_json::to_value(value).unwrap_or(serde_json::Value::Null);
    let mut params = Vec::new();
    if let Some(obj) = json.as_object() {
        for f in &insert_fields {
            let v = obj.get(f.rust_name).cloned().unwrap_or(serde_json::Value::Null);
            params.push(v);
        }
    }

    (sql, params)
}

#[cfg(feature = "postgres")]
async fn sqlx_insert_postgres<T: Model + serde::de::DeserializeOwned>(
    pool: &sqlx::PgPool,
    value: &T,
) -> Result<T> {
    let (sql, params) = build_insert_sql(value);
    let mut q = sqlx::query(&sql);
    for p in &params {
        q = bind_param_postgres(q, p);
    }
    let rows = q.fetch_all(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    if let Some(row) = rows.into_iter().next() {
        let value = row_to_json_postgres(&row);
        let t: T = serde_json::from_value(value).map_err(Error::Serde)?;
        Ok(t)
    } else {
        Err(Error::Database("INSERT returned no rows".into()))
    }
}

#[cfg(feature = "mysql")]
async fn sqlx_insert_mysql<T: Model + serde::de::DeserializeOwned>(
    pool: &sqlx::MySqlPool,
    value: &T,
) -> Result<T> {
    // MySQL doesn't support RETURNING — insert + return the original value.
    let (sql, params) = build_insert_sql(value);
    let mut q = sqlx::query(&sql);
    for p in &params {
        q = bind_param_mysql(q, p);
    }
    q.execute(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    // Re-deserialise from the original value (PK won't be populated for
    // auto-increment — caller should fetch it via last_insert_id()).
    serde_json::from_value(serde_json::to_value(value).unwrap_or(serde_json::Value::Null))
        .map_err(Error::Serde)
}

#[cfg(feature = "sqlite")]
async fn sqlx_insert_sqlite<T: Model + serde::de::DeserializeOwned>(
    pool: &sqlx::SqlitePool,
    value: &T,
) -> Result<T> {
    // SQLite >= 3.35 supports RETURNING — use it if available.
    let (mut sql, params) = build_insert_sql(value);
    if !sql.to_uppercase().contains("RETURNING") {
        sql.push_str(" RETURNING *");
    }
    let mut q = sqlx::query(&sql);
    for p in &params {
        q = bind_param_sqlite(q, p);
    }
    let rows = q.fetch_all(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    if let Some(row) = rows.into_iter().next() {
        let value = row_to_json_sqlite(&row);
        let t: T = serde_json::from_value(value).map_err(Error::Serde)?;
        Ok(t)
    } else {
        // Fallback: return the original value.
        serde_json::from_value(serde_json::to_value(value).unwrap_or(serde_json::Value::Null))
            .map_err(Error::Serde)
    }
}

// ─── sqlx COUNT implementations ───────────────────────────────────────────────

#[cfg(feature = "postgres")]
async fn sqlx_count_postgres(
    pool: &sqlx::PgPool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<i64> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param_postgres(q, p); }
    let row = q.fetch_one(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    use sqlx::Row;
    let count: i64 = row.try_get("count").unwrap_or(0);
    Ok(count)
}

#[cfg(feature = "mysql")]
async fn sqlx_count_mysql(
    pool: &sqlx::MySqlPool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<i64> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param_mysql(q, p); }
    let row = q.fetch_one(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    use sqlx::Row;
    let count: i64 = row.try_get("count").unwrap_or(0);
    Ok(count)
}

#[cfg(feature = "sqlite")]
async fn sqlx_count_sqlite(
    pool: &sqlx::SqlitePool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<i64> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param_sqlite(q, p); }
    let row = q.fetch_one(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    use sqlx::Row;
    let count: i64 = row.try_get("count").unwrap_or(0);
    Ok(count)
}

// ─── sqlx EXECUTE implementations (UPDATE/DELETE) ─────────────────────────────

#[cfg(feature = "postgres")]
async fn sqlx_execute_postgres(
    pool: &sqlx::PgPool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<u64> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param_postgres(q, p); }
    let result = q.execute(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok(result.rows_affected())
}

#[cfg(feature = "mysql")]
async fn sqlx_execute_mysql(
    pool: &sqlx::MySqlPool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<u64> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param_mysql(q, p); }
    let result = q.execute(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok(result.rows_affected())
}

#[cfg(feature = "sqlite")]
async fn sqlx_execute_sqlite(
    pool: &sqlx::SqlitePool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<u64> {
    let mut q = sqlx::query(sql);
    for p in params { q = bind_param_sqlite(q, p); }
    let result = q.execute(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok(result.rows_affected())
}
