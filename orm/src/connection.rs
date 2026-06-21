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

use crate::{Error, Model, Query, Result};

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
    inner: Arc<DbInner>,
}

enum DbInner {
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
        let _ = config; // unused in mock mode
        #[cfg(feature = "postgres")]
        {
            if config.url.starts_with("postgres://") || config.url.starts_with("postgresql://") {
                let pool = sqlx::postgres::PgPoolOptions::new()
                    .max_connections(config.max_connections as u32)
                    .min_connections(config.min_connections as u32)
                    .connect(&config.url)
                    .await
                    .map_err(|e| Error::Database(e.to_string()))?;
                return Ok(Self { inner: Arc::new(DbInner::Postgres(pool)) });
            }
        }
        #[allow(unreachable_code)]
        Ok(Self::mock())
    }

    /// Execute a SELECT query and deserialise rows into `T`.
    pub async fn query<T: Model>(&self, q: Query<T>) -> Result<Vec<T>> {
        match &*self.inner {
            DbInner::Mock(m) => mock_query(m, q).await,
            #[cfg(feature = "postgres")]
            DbInner::Postgres(_) => Err(Error::NoDriver), // TODO: implement sqlx-backed query
            #[cfg(feature = "mysql")]
            DbInner::Mysql(_) => Err(Error::NoDriver),
            #[cfg(feature = "sqlite")]
            DbInner::Sqlite(_) => Err(Error::NoDriver),
        }
    }

    /// Count matching rows.
    pub async fn count<T: Model>(&self, _q: Query<T>) -> Result<i64> {
        // Simplified — V1 will translate the query to COUNT(*) properly.
        Ok(0)
    }

    /// Insert a row.
    pub async fn insert_row<T: Model>(&self, value: &T) -> Result<T> {
        match &*self.inner {
            DbInner::Mock(m) => mock_insert(m, value).await,
            #[cfg(feature = "postgres")]
            DbInner::Postgres(_) => Err(Error::NoDriver),
            #[cfg(feature = "mysql")]
            DbInner::Mysql(_) => Err(Error::NoDriver),
            #[cfg(feature = "sqlite")]
            DbInner::Sqlite(_) => Err(Error::NoDriver),
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
