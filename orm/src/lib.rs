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

pub use connection::{Db, DbConfig};
pub use error::{Error, Result};
pub use migrations::{generate_migration, Migration};
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
}
