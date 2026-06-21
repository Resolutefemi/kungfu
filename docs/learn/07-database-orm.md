# Database & ORM

> ⏱️ 10 minutes

Kungfu ships with a built-in ORM (`kungfu-orm`) that uses parameterised
queries (SQL-injection-proof), auto-hashes passwords with Argon2id, and
generates migrations from your model definitions.

## Defining models

Use `#[derive(Model)]` with `#[field]` attributes:

```rust
use kungfu_macros::Model;
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize)]
#[table(name = "users")]
struct User {
    #[field(primary, auto_increment)]
    id: i64,
    #[field(unique)]
    email: String,
    #[field(min = 8, sensitive)]  // sensitive = auto-hashed with Argon2id
    password: String,
    #[field(skip)]  // not persisted to the database
    computed_field: String,
}
```

### Field attributes

| Attribute | Effect |
|---|---|
| `primary` | Marks this as the primary key |
| `auto_increment` | Auto-incrementing primary key (BIGINT) |
| `unique` | Adds a UNIQUE constraint |
| `sensitive` | Auto-hashed with Argon2id on insert |
| `min = N` | CHECK (length(field) >= N) |
| `max = N` | CHECK (length(field) <= N) |
| `skip` | Field is not persisted |

## Connecting

V1 ships with a mock in-process driver (no setup required, great for
tests). Real Postgres / MySQL / SQLite drivers are feature-gated:

```toml
[dependencies]
kungfu-orm = { path = "../orm", features = ["postgres"] }
```

```rust
use kungfu_orm::{Db, DbConfig};

// Mock driver — no setup:
let db = Db::mock();

// Real Postgres:
let db = Db::connect(&DbConfig {
    url: "postgres://user:pass@localhost/mydb".into(),
    max_connections: 10,
    min_connections: 2,
}).await?;
```

## Inserting

```rust
let alice = User {
    id: 0,
    email: "alice@example.com".into(),
    password: "hunter222".into(),  // plaintext — auto-hashed on insert
    computed_field: "ignored".into(),
};
let alice = User::insert(&alice, &db).await?;
println!("Inserted user with id={}", alice.id);
// alice.password is now an Argon2id hash, not the plaintext.
```

## Querying

The query builder is type-safe and parameterised:

```rust
// Find all users.
let all_users = User::all(&db).await?;

// Find one user by email.
let alice = User::find()
    .where_eq("email", "alice@example.com")
    .one(&db)
    .await?;

// Find with multiple conditions + ordering + pagination.
let active_users: Vec<User> = User::find()
    .where_gt("id", 5)
    .where_in("status", vec!["active", "pending"])
    .order_desc("created_at")
    .limit(10)
    .offset(20)
    .all(&db)
    .await?;
```

### Where clauses

| Method | SQL |
|---|---|
| `.where_eq(col, v)` | `col = $1` |
| `.where_ne(col, v)` | `col <> $1` |
| `.where_lt(col, v)` | `col < $1` |
| `.where_le(col, v)` | `col <= $1` |
| `.where_gt(col, v)` | `col > $1` |
| `.where_ge(col, v)` | `col >= $1` |
| `.where_like(col, pattern)` | `col LIKE $1` |
| `.where_null(col)` | `col IS NULL` |
| `.where_not_null(col)` | `col IS NOT NULL` |
| `.where_in(col, vec)` | `col IN ($1, $2, $3)` |

All values go through parameterised placeholders — never string-interpolated
into the SQL. This is the framework's primary SQL-injection defence.

## Verifying passwords

```rust
use kungfu_orm::password::verify_password;

// On login:
let user = User::find()
    .where_eq("email", email_from_form)
    .one(&db)
    .await?;

match user {
    Some(u) if verify_password(&plaintext_from_form, &u.password)? => {
        // Login successful.
    }
    _ => {
        // Invalid email or password.
    }
}
```

## Migrations

Generate a `CREATE TABLE` migration from your model definition:

```rust
use kungfu_orm::generate_migration;

let migration = generate_migration::<User>();
println!("Migration: {}", migration.name);
for stmt in &migration.up_sql {
    println!("{}", stmt);
}
```

Output:

```sql
CREATE TABLE IF NOT EXISTS users (
  id BIGINT PRIMARY KEY AUTOINCREMENT,
  email TEXT UNIQUE,
  password VARCHAR(255),
  CHECK (length(password) >= 8)
);
```

## Next steps

Continue to [Frontend & SSR](./08-frontend-ssr.md) to learn about `.kungfu`
files and live reload.
