//! Built-in ORM for the Kungfu.js framework.
//!
//! The ORM is woven into the framework — there's no separate "ORM crate"
//! to install. Models are defined with `#[derive(Model)]` and queried via
//! the `Query` builder. Passwords marked `sensitive` are auto-hashed with
//! Argon2id; queries are parameterised to prevent SQL injection.
//!
//! ## Example
//!
//! ```ignore
//! use kungfu_orm::{Model, Query};
//! use serde::{Serialize, Deserialize};
//!
//! #[derive(Model, Serialize, Deserialize)]
//! #[table(name = "users")]
//! struct User {
//!     #[field(primary, auto_increment)]
//!     id: i64,
//!     #[field(unique)]
//!     email: String,
//!     #[field(min = 8, sensitive)]
//!     password: String,
//! }
//!
//! async fn example(db: &kungfu_orm::Db) -> Result<(), kungfu_orm::Error> {
//!     // Create
//!     let user = User { id: 0, email: "a@b.com".into(), password: "hunter22".into() };
//!     let user = User::insert(&user, db).await?;
//!
//!     // Query
//!     let found = User::find()
//!         .where_eq("email", "a@b.com")
//!         .one(db)
//!         .await?;
//!     Ok(())
//! }
//! ```

// Allow `#[derive(Model)]` to refer to `kungfu_orm` from inside this crate's
// own tests.
#[cfg(test)]
extern crate self as kungfu_orm;

pub mod query;
pub mod connection;
pub mod error;
pub mod migrations;
pub mod password;
pub mod extensions;

pub use connection::{Db, DbConfig};
pub use error::{Error, Result};
pub use extensions::{JoinClause, JoinType, Transaction};
pub use migrations::{generate_migration, Migration};
pub use password::{hash_password, verify_password};
pub use query::{Query, WhereClause};

/// Static metadata about a model field, collected by `#[derive(Model)]`.
#[derive(Debug, Clone, Copy)]
pub struct FieldDef {
    pub rust_name: &'static str,
    pub column_name: &'static str,
    pub is_primary: bool,
    pub auto_increment: bool,
    pub unique: bool,
    pub sensitive: bool,
    pub min_len: Option<usize>,
    pub max_len: Option<usize>,
}

/// Trait implemented by all ORM models via `#[derive(Model)]`.
pub trait Model: serde::Serialize + serde::de::DeserializeOwned + Send + Sync + 'static {
    fn table_name() -> &'static str;
    fn fields() -> &'static [FieldDef];

    /// Build a query selecting rows from this model's table.
    fn find() -> Query<Self> {
        Query::select(Self::table_name())
    }

    /// Insert a row.
    async fn insert(&self, db: &Db) -> Result<Self> {
        db.insert_row::<Self>(self).await
    }

    /// Fetch all rows.
    async fn all(db: &Db) -> Result<Vec<Self>> {
        Query::<Self>::select(Self::table_name()).all(db).await
    }

    /// Find a row by primary key. Returns `NotFound` if missing.
    /// Only works for models with a single primary key field.
    async fn find_by_pk<V: serde::Serialize + Send + Sync>(pk: V, db: &Db) -> Result<Self> {
        let pk_field = Self::fields().iter().find(|f| f.is_primary).ok_or(Error::Database(
            "no primary key field defined on model".into()
        ))?;
        Self::find()
            .where_eq(pk_field.column_name, pk)
            .one(db)
            .await
    }

    /// Update this row by primary key. `sets` is a list of (column, new_value).
    /// The PK column is auto-appended to the WHERE clause.
    async fn update_by_pk<V: serde::Serialize + Send + Sync>(
        db: &Db,
        pk_value: V,
        sets: Vec<(&'static str, serde_json::Value)>,
    ) -> Result<u64> {
        let pk_field = Self::fields().iter().find(|f| f.is_primary).ok_or(Error::Database(
            "no primary key field defined on model".into()
        ))?;
        let mut q = Query::<Self>::select(Self::table_name());
        q = q.where_eq(pk_field.column_name, pk_value);
        let sets: Vec<(String, serde_json::Value)> = sets
            .into_iter()
            .map(|(c, v)| (c.to_string(), v))
            .collect();
        let (sql, params) = q.to_update_sql(&sets);
        db.execute(&sql, &params).await
    }

    /// Delete rows matching the given WHERE column + value.
    async fn delete_where<V: serde::Serialize + Send + Sync>(
        col: &'static str,
        value: V,
        db: &Db,
    ) -> Result<u64> {
        let mut q = Query::<Self>::select(Self::table_name());
        q = q.where_eq(col, value);
        let (sql, params) = q.to_delete_sql();
        db.execute(&sql, &params).await
    }

    /// Delete a row by primary key. Returns the number of rows deleted.
    async fn delete_by_pk<V: serde::Serialize + Send + Sync>(pk_value: V, db: &Db) -> Result<u64> {
        let pk_field = Self::fields().iter().find(|f| f.is_primary).ok_or(Error::Database(
            "no primary key field defined on model".into()
        ))?;
        Self::delete_where(pk_field.column_name, pk_value, db).await
    }

    /// Count rows matching the current query.
    async fn count(db: &Db) -> Result<i64> {
        Query::<Self>::select(Self::table_name()).count(db).await
    }
}
