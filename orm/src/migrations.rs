//! Schema migration generator.
//!
//! Compares the current `Model` definitions to the database schema and
//! emits SQL to bring the database up to date. V1 supports create-table
//! migrations only; ALTER TABLE support is planned for V2.

use crate::{FieldDef, Model};

/// A single migration: one or more SQL statements to apply.
#[derive(Debug, Clone)]
pub struct Migration {
    pub name: String,
    pub up_sql: Vec<String>,
    pub down_sql: Vec<String>,
}

/// Generate a CREATE TABLE migration for a model.
pub fn generate_migration<T: Model>() -> Migration {
    let table = T::table_name();
    let mut up_sql = Vec::new();
    let mut down_sql = Vec::new();

    let columns: Vec<String> = T::fields()
        .iter()
        .map(|f| column_def(f))
        .collect();

    up_sql.push(format!(
        "CREATE TABLE IF NOT EXISTS {} (\n  {}\n);",
        table,
        columns.join(",\n  ")
    ));
    down_sql.push(format!("DROP TABLE IF EXISTS {};", table));

    Migration {
        name: format!("create_{}", table),
        up_sql,
        down_sql,
    }
}

fn column_def(f: &FieldDef) -> String {
    let mut parts = vec![f.column_name.to_string(), sql_type_for(f).to_string()];

    if f.is_primary {
        parts.push("PRIMARY KEY".to_string());
    }
    if f.auto_increment {
        parts.push("AUTOINCREMENT".to_string());
    }
    if f.unique {
        parts.push("UNIQUE".to_string());
    }
    if let Some(n) = f.min_len {
        parts.push(format!("CHECK (length({}) >= {})", f.column_name, n));
    }
    if let Some(n) = f.max_len {
        parts.push(format!("CHECK (length({}) <= {})", f.column_name, n));
    }

    parts.join(" ")
}

fn sql_type_for(f: &FieldDef) -> &'static str {
    // The proc macro doesn't have access to the field's Rust type at
    // codegen time (well, it does, but we kept the FieldDef minimal). For
    // V2 we'll thread the Rust type through FieldDef. For V1 we infer the
    // SQL type from heuristics:
    //   - primary keys → BIGINT
    //   - sensitive fields (passwords) → VARCHAR(255)
    //   - everything else → TEXT
    if f.is_primary {
        "BIGINT"
    } else if f.sensitive {
        "VARCHAR(255)"
    } else {
        "TEXT"
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
    fn generates_create_table_migration() {
        let m = generate_migration::<User>();
        assert_eq!(m.name, "create_users");
        assert_eq!(m.up_sql.len(), 1);
        assert!(m.up_sql[0].contains("CREATE TABLE IF NOT EXISTS users"));
        assert!(m.up_sql[0].contains("id BIGINT PRIMARY KEY AUTOINCREMENT"));
        assert!(m.up_sql[0].contains("email TEXT UNIQUE"));
        assert!(m.up_sql[0].contains("password VARCHAR(255)"));
        assert!(m.up_sql[0].contains("CHECK (length(password) >= 8)"));
        assert_eq!(m.down_sql[0], "DROP TABLE IF EXISTS users;");
    }
}
