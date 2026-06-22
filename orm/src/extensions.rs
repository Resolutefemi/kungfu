//! ORM extensions: transactions, JOINs, aggregates.
//!
//! Extends the query builder with:
//! - `transaction()` — BEGIN/COMMIT/ROLLBACK
//! - `join()` — INNER JOIN / LEFT JOIN
//! - `aggregate()` — COUNT/SUM/AVG/MIN/MAX with GROUP BY

use crate::{Db, Error, Result};
use serde::de::DeserializeOwned;

impl Db {
    /// Execute a closure inside a database transaction.
    ///
    /// If the closure returns `Ok`, the transaction is committed.
    /// If it returns `Err`, the transaction is rolled back.
    ///
    /// ```ignore
    /// db.transaction(|tx| async move {
    ///     tx.execute("INSERT INTO users (email) VALUES ($1)", &[json!("a@b.com")]).await?;
    ///     tx.execute("INSERT INTO logs (msg) VALUES ($1)", &[json!("user created")]).await?;
    ///     Ok(())
    /// }).await?;
    /// ```
    pub async fn transaction<F, Fut, T>(&self, f: F) -> Result<T>
    where
        F: FnOnce(Transaction) -> Fut + Send,
        Fut: std::future::Future<Output = Result<T>> + Send,
        T: Send,
    {
        // BEGIN
        self.execute("BEGIN", &[]).await?;

        let tx = Transaction { db: self.clone() };
        let result = f(tx).await;

        match result {
            Ok(val) => {
                // COMMIT
                self.execute("COMMIT", &[]).await?;
                Ok(val)
            }
            Err(e) => {
                // ROLLBACK
                let _ = self.execute("ROLLBACK", &[]).await;
                Err(e)
            }
        }
    }

    /// Execute a raw SQL query and return the result as JSON values.
    /// Useful for JOINs and aggregates that don't map to a single Model.
    pub async fn query_raw(&self, sql: &str, params: &[serde_json::Value]) -> Result<Vec<serde_json::Value>> {
        // Reuse the existing query infrastructure — fetch rows as JSON.
        // For V1, we use the same row-to-json conversion as Model queries.
        #[cfg(feature = "sqlite")]
        if let crate::connection::DbInner::Sqlite(pool) = &*self.inner {
            return sqlite_raw_query(pool, sql, params).await;
        }
        #[cfg(feature = "postgres")]
        if let crate::connection::DbInner::Postgres(pool) = &*self.inner {
            return postgres_raw_query(pool, sql, params).await;
        }
        // Mock driver: return empty.
        Ok(Vec::new())
    }

    /// Execute an aggregate query (COUNT/SUM/AVG/MIN/MAX).
    ///
    /// ```ignore
    /// let total: i64 = db.aggregate("SELECT COUNT(*) FROM users WHERE active = $1", &[json!(true)]).await?;
    /// let sum: f64 = db.aggregate("SELECT SUM(price) FROM orders", &[]).await?;
    /// ```
    pub async fn aggregate(&self, sql: &str, params: &[serde_json::Value]) -> Result<f64> {
        let rows = self.query_raw(sql, params).await?;
        if let Some(row) = rows.first() {
            // The aggregate value is the first column of the first row.
            if let Some(obj) = row.as_object() {
                if let Some((_, v)) = obj.iter().next() {
                    return Ok(v.as_f64().unwrap_or(0.0));
                }
            }
            return Ok(row.as_f64().unwrap_or(0.0));
        }
        Ok(0.0)
    }
}

/// A database transaction. Wraps a `Db` reference.
pub struct Transaction {
    db: Db,
}

impl Transaction {
    /// Execute a SQL statement within this transaction.
    pub async fn execute(&self, sql: &str, params: &[serde_json::Value]) -> Result<u64> {
        self.db.execute(sql, params).await
    }

    /// Execute a SELECT within this transaction.
    pub async fn query_raw(&self, sql: &str, params: &[serde_json::Value]) -> Result<Vec<serde_json::Value>> {
        self.db.query_raw(sql, params).await
    }
}

// ─── JOIN support in Query ────────────────────────────────────────────────────

/// A JOIN clause for the query builder.
#[derive(Debug, Clone)]
pub struct JoinClause {
    pub join_type: JoinType,
    pub table: String,
    pub on: String, // e.g. "users.id = posts.user_id"
}

#[derive(Debug, Clone, Copy)]
pub enum JoinType {
    Inner,
    Left,
    Right,
}

impl JoinType {
    fn as_sql(&self) -> &'static str {
        match self {
            JoinType::Inner => "INNER JOIN",
            JoinType::Left => "LEFT JOIN",
            JoinType::Right => "RIGHT JOIN",
        }
    }
}

// ─── Raw query implementations ────────────────────────────────────────────────

#[cfg(feature = "sqlite")]
async fn sqlite_raw_query(
    pool: &sqlx::SqlitePool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<Vec<serde_json::Value>> {
    use crate::connection::{bind_param_sqlite, row_to_json_sqlite};
    use sqlx::Row;
    let mut q = sqlx::query(sql);
    for p in params {
        q = bind_param_sqlite(q, p);
    }
    let rows = q.fetch_all(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok(rows.iter().map(row_to_json_sqlite).collect())
}

#[cfg(feature = "postgres")]
async fn postgres_raw_query(
    pool: &sqlx::PgPool,
    sql: &str,
    params: &[serde_json::Value],
) -> Result<Vec<serde_json::Value>> {
    use crate::connection::{bind_param_postgres, row_to_json_postgres};
    use sqlx::Row;
    let mut q = sqlx::query(sql);
    for p in params {
        q = bind_param_postgres(q, p);
    }
    let rows = q.fetch_all(pool).await.map_err(|e| Error::Database(e.to_string()))?;
    Ok(rows.iter().map(row_to_json_postgres).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn join_type_sql() {
        assert_eq!(JoinType::Inner.as_sql(), "INNER JOIN");
        assert_eq!(JoinType::Left.as_sql(), "LEFT JOIN");
        assert_eq!(JoinType::Right.as_sql(), "RIGHT JOIN");
    }
}
