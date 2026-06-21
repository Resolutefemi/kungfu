//! Integration test: ORM with real SQLite via sqlx.
//!
//! Run with:
//!   cargo test -p kungfu-orm --features sqlite --test sqlite_integration -- --nocapture

use kungfu_macros::Model;
use kungfu_orm::{Db, DbConfig, Model as ModelTrait};
use serde::{Deserialize, Serialize};

#[derive(Model, Serialize, Deserialize)]
#[table(name = "users")]
struct User {
    #[field(primary, auto_increment)]
    id: i64,
    #[field(unique)]
    email: String,
    name: String,
}

async fn setup_db() -> Db {
    // Use an in-memory SQLite database.
    let db = Db::connect(&DbConfig {
        url: "sqlite::memory:".into(),
        max_connections: 1,
        min_connections: 0,
    })
    .await
    .expect("connect");

    // Create the table.
    let migration = kungfu_orm::generate_migration::<User>();
    for stmt in &migration.up_sql {
        db.execute(stmt, &[]).await.expect("create table");
    }

    db
}

#[tokio::test]
async fn test_insert_and_select() {
    let db = setup_db().await;

    // Insert a user.
    let alice = User {
        id: 0,
        email: "alice@example.com".into(),
        name: "Alice".into(),
    };
    let inserted = alice.insert(&db).await.expect("insert");
    assert!(inserted.id > 0, "id should be auto-assigned");
    assert_eq!(inserted.email, "alice@example.com");

    // Insert another.
    let bob = User {
        id: 0,
        email: "bob@example.com".into(),
        name: "Bob".into(),
    };
    let _ = bob.insert(&db).await.expect("insert bob");

    // Count.
    let count = User::count(&db).await.expect("count");
    assert_eq!(count, 2);

    // Select all.
    let all = User::all(&db).await.expect("all");
    assert_eq!(all.len(), 2);

    // Find by PK.
    let found = User::find_by_pk(inserted.id, &db).await.expect("find by pk");
    assert_eq!(found.email, "alice@example.com");
}

#[tokio::test]
async fn test_update_and_delete() {
    let db = setup_db().await;

    // Insert.
    let user = User {
        id: 0,
        email: "carol@example.com".into(),
        name: "Carol".into(),
    };
    let inserted = user.insert(&db).await.expect("insert");
    let pk = inserted.id;

    // Update.
    let affected = User::update_by_pk(
        &db,
        pk,
        vec![("name", serde_json::json!("Carol Updated"))],
    )
    .await
    .expect("update");
    assert_eq!(affected, 1);

    // Verify update.
    let updated = User::find_by_pk(pk, &db).await.expect("find after update");
    assert_eq!(updated.name, "Carol Updated");

    // Delete.
    let deleted = User::delete_by_pk(pk, &db).await.expect("delete");
    assert_eq!(deleted, 1);

    // Verify deletion.
    let result = User::find_by_pk(pk, &db).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn test_count_with_where() {
    let db = setup_db().await;

    let _ = User {
        id: 0,
        email: "a@x.com".into(),
        name: "Alice".into(),
    }
    .insert(&db)
    .await
    .unwrap();

    let _ = User {
        id: 0,
        email: "b@x.com".into(),
        name: "Bob".into(),
    }
    .insert(&db)
    .await
    .unwrap();

    let _ = User {
        id: 0,
        email: "c@x.com".into(),
        name: "Alice".into(),  // same name as first
    }
    .insert(&db)
    .await
    .unwrap();

    // Count all.
    let total = User::count(&db).await.unwrap();
    assert_eq!(total, 3);

    // Count with WHERE.
    let alices = User::find()
        .where_eq("name", "Alice")
        .count(&db)
        .await
        .unwrap();
    assert_eq!(alices, 2);
}
