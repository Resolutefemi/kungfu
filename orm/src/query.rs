//! Type-safe query builder.
//!
//! All SQL is parameterised — values go into a separate `params` Vec, never
//! string-interpolated into the SQL itself. This is the framework's primary
//! SQL-injection defence.

use std::marker::PhantomData;

use crate::{Db, Error, Model, Result};

/// A single WHERE clause.
#[derive(Debug, Clone)]
pub enum WhereClause {
    Eq(String, serde_json::Value),
    Ne(String, serde_json::Value),
    Lt(String, serde_json::Value),
    Le(String, serde_json::Value),
    Gt(String, serde_json::Value),
    Ge(String, serde_json::Value),
    Like(String, String),
    IsNull(String),
    IsNotNull(String),
    In(String, Vec<serde_json::Value>),
}

/// A typed query. `T` is the model type the query will return.
pub struct Query<T: Model> {
    table: String,
    wheres: Vec<WhereClause>,
    order_by: Option<(String, bool)>, // (column, descending)
    limit: Option<usize>,
    offset: Option<usize>,
    _phantom: PhantomData<T>,
}

impl<T: Model> Query<T> {
    pub fn select(table: &str) -> Self {
        Self {
            table: table.to_string(),
            wheres: Vec::new(),
            order_by: None,
            limit: None,
            offset: None,
            _phantom: PhantomData,
        }
    }

    pub fn where_eq<V: serde::Serialize>(mut self, col: &str, val: V) -> Self {
        let v = serde_json::to_value(val).expect("where_eq serialisation");
        self.wheres.push(WhereClause::Eq(col.to_string(), v));
        self
    }
    pub fn where_ne<V: serde::Serialize>(mut self, col: &str, val: V) -> Self {
        let v = serde_json::to_value(val).expect("where_ne serialisation");
        self.wheres.push(WhereClause::Ne(col.to_string(), v));
        self
    }
    pub fn where_lt<V: serde::Serialize>(mut self, col: &str, val: V) -> Self {
        let v = serde_json::to_value(val).expect("where_lt serialisation");
        self.wheres.push(WhereClause::Lt(col.to_string(), v));
        self
    }
    pub fn where_gt<V: serde::Serialize>(mut self, col: &str, val: V) -> Self {
        let v = serde_json::to_value(val).expect("where_gt serialisation");
        self.wheres.push(WhereClause::Gt(col.to_string(), v));
        self
    }
    pub fn where_like(mut self, col: &str, pattern: impl Into<String>) -> Self {
        self.wheres.push(WhereClause::Like(col.to_string(), pattern.into()));
        self
    }
    pub fn where_null(mut self, col: &str) -> Self {
        self.wheres.push(WhereClause::IsNull(col.to_string()));
        self
    }
    pub fn where_not_null(mut self, col: &str) -> Self {
        self.wheres.push(WhereClause::IsNotNull(col.to_string()));
        self
    }
    pub fn where_in<V: serde::Serialize>(mut self, col: &str, vals: Vec<V>) -> Self {
        let vs: Vec<_> = vals.into_iter().map(|v| serde_json::to_value(v).unwrap()).collect();
        self.wheres.push(WhereClause::In(col.to_string(), vs));
        self
    }

    pub fn order_desc(mut self, col: &str) -> Self {
        self.order_by = Some((col.to_string(), true));
        self
    }
    pub fn order_asc(mut self, col: &str) -> Self {
        self.order_by = Some((col.to_string(), false));
        self
    }

    pub fn limit(mut self, n: usize) -> Self {
        self.limit = Some(n);
        self
    }
    pub fn offset(mut self, n: usize) -> Self {
        self.offset = Some(n);
        self
    }

    /// Compile this query into (SQL, params).
    pub fn to_sql(&self) -> (String, Vec<serde_json::Value>) {
        let mut sql = format!("SELECT * FROM {}", self.table);
        let mut params = Vec::new();

        if !self.wheres.is_empty() {
            sql.push_str(" WHERE ");
            let mut parts = Vec::new();
            for w in &self.wheres {
                let next_idx = params.len() + 1;
                let (clause, mut p) = compile_where(w, next_idx);
                parts.push(clause);
                params.append(&mut p);
            }
            sql.push_str(&parts.join(" AND "));
        }

        if let Some((col, desc)) = &self.order_by {
            sql.push_str(&format!(" ORDER BY {} {}", col, if *desc { "DESC" } else { "ASC" }));
        }
        if let Some(n) = self.limit {
            sql.push_str(&format!(" LIMIT {}", n));
        }
        if let Some(n) = self.offset {
            sql.push_str(&format!(" OFFSET {}", n));
        }

        (sql, params)
    }

    /// Execute the query and return all matching rows.
    pub async fn all(self, db: &Db) -> Result<Vec<T>> {
        db.query::<T>(self).await
    }

    /// Execute the query and return the first matching row, or `NotFound`.
    pub async fn one(mut self, db: &Db) -> Result<T> {
        self.limit = Some(1);
        let mut rows = db.query::<T>(self).await?;
        rows.into_iter().next().ok_or(Error::NotFound)
    }

    /// Count matching rows (drops LIMIT/OFFSET).
    pub async fn count(self, db: &Db) -> Result<i64> {
        db.count(self).await
    }
}

fn compile_where(w: &WhereClause, start_idx: usize) -> (String, Vec<serde_json::Value>) {
    match w {
        WhereClause::Eq(c, v) => (format!("{c} = ${start_idx}"), vec![v.clone()]),
        WhereClause::Ne(c, v) => (format!("{c} <> ${start_idx}"), vec![v.clone()]),
        WhereClause::Lt(c, v) => (format!("{c} < ${start_idx}"), vec![v.clone()]),
        WhereClause::Le(c, v) => (format!("{c} <= ${start_idx}"), vec![v.clone()]),
        WhereClause::Gt(c, v) => (format!("{c} > ${start_idx}"), vec![v.clone()]),
        WhereClause::Ge(c, v) => (format!("{c} >= ${start_idx}"), vec![v.clone()]),
        WhereClause::Like(c, v) => (format!("{c} LIKE ${start_idx}"), vec![serde_json::Value::String(v.clone())]),
        WhereClause::IsNull(c) => (format!("{c} IS NULL"), vec![]),
        WhereClause::IsNotNull(c) => (format!("{c} IS NOT NULL"), vec![]),
        WhereClause::In(c, vs) => {
            if vs.is_empty() {
                ("FALSE".to_string(), vec![])
            } else {
                let placeholders: Vec<String> = (0..vs.len()).map(|i| format!("${}", start_idx + i)).collect();
                (format!("{c} IN ({})", placeholders.join(", ")), vs.clone())
            }
        }
    }
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
        #[field(unique)]
        email: String,
        #[field(min = 8, sensitive)]
        password: String,
    }

    #[test]
    fn compiles_select_all() {
        let q: Query<User> = Query::select("users");
        let (sql, params) = q.to_sql();
        assert_eq!(sql, "SELECT * FROM users");
        assert!(params.is_empty());
    }

    #[test]
    fn compiles_where_eq() {
        let q: Query<User> = Query::select("users").where_eq("email", "a@b.com");
        let (sql, params) = q.to_sql();
        assert_eq!(sql, "SELECT * FROM users WHERE email = $1");
        assert_eq!(params.len(), 1);
        assert_eq!(params[0], serde_json::Value::String("a@b.com".to_string()));
    }

    #[test]
    fn compiles_where_in() {
        let q: Query<User> = Query::select("users").where_in("id", vec![1, 2, 3]);
        let (sql, params) = q.to_sql();
        assert_eq!(sql, "SELECT * FROM users WHERE id IN ($1, $2, $3)");
        assert_eq!(params.len(), 3);
    }

    #[test]
    fn compiles_multiple_wheres() {
        let q: Query<User> = Query::select("users")
            .where_eq("email", "a@b.com")
            .where_gt("id", 5);
        let (sql, params) = q.to_sql();
        assert_eq!(sql, "SELECT * FROM users WHERE email = $1 AND id > $2");
        assert_eq!(params.len(), 2);
    }

    #[test]
    fn compiles_order_and_limit() {
        let q: Query<User> = Query::select("users")
            .order_desc("created_at")
            .limit(10)
            .offset(20);
        let (sql, _) = q.to_sql();
        assert_eq!(sql, "SELECT * FROM users ORDER BY created_at DESC LIMIT 10 OFFSET 20");
    }
}
