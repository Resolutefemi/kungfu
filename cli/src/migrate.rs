//! `kungfu migrate` — generate + apply SQL migrations.
//!
//! Scans the project's `src/` for `#[derive(Model)]` structs and generates
//! `CREATE TABLE` migrations for each. In `--apply` mode, executes them
//! against a real database via sqlx (requires the matching feature).

use std::path::PathBuf;

/// Generate migrations for all Model definitions in the project.
///
/// This is a stub — in V1 we don't have a way to enumerate types at runtime
/// in Rust. The actual migration generation happens via `kungfu_orm::generate_migration::<T>()`
/// called explicitly by the user in their `main.rs`.
pub fn generate_migrations(_project_root: &PathBuf) -> Vec<String> {
    // V1: we just emit guidance. The user calls generate_migration::<T>()
    // in their code.
    vec![
        "-- Kungfu migration generator".to_string(),
        "-- Add migrations by calling kungfu_orm::generate_migration::<YourModel>()".to_string(),
        "-- in your main.rs, then execute the returned SQL against your database.".to_string(),
    ]
}

/// Apply a list of SQL statements to a database.
///
/// Requires the matching sqlx feature (`postgres`, `mysql`, or `sqlite`).
#[cfg(any(feature = "postgres", feature = "mysql", feature = "sqlite"))]
pub async fn apply_migrations(url: &str, sqls: &[String]) -> Result<(), String> {
    #[cfg(feature = "postgres")]
    if url.starts_with("postgres://") || url.starts_with("postgresql://") {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(url)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        for sql in sqls {
            sqlx::query(sql)
                .execute(&pool)
                .await
                .map_err(|e| format!("execute: {e}"))?;
            println!("  ✓ Applied: {}", sql.lines().next().unwrap_or(""));
        }
        return Ok(());
    }
    #[cfg(feature = "mysql")]
    if url.starts_with("mysql://") {
        let pool = sqlx::mysql::MySqlPoolOptions::new()
            .max_connections(1)
            .connect(url)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        for sql in sqls {
            sqlx::query(sql).execute(&pool).await.map_err(|e| format!("execute: {e}"))?;
        }
        return Ok(());
    }
    #[cfg(feature = "sqlite")]
    if url.starts_with("sqlite://") {
        let pool = sqlx::sqlite::SqlitePoolOptions::new()
            .max_connections(1)
            .connect(url)
            .await
            .map_err(|e| format!("connect: {e}"))?;
        for sql in sqls {
            sqlx::query(sql).execute(&pool).await.map_err(|e| format!("execute: {e}"))?;
        }
        return Ok(());
    }
    Err(format!("unsupported database URL: {url}"))
}
