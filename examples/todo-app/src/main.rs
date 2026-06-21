//! Complete Kungfu.js Todo App — demonstrates every feature.
//!
//! Run: cargo run --example todo-app
//! Visit: http://localhost:3000

use kungfu::prelude::*;
use kungfu_core::auth::{JwtService, JwtConfig, auth_jwt};
use kungfu_core::websocket::{WebSocket, WebSocketMessage};
use kungfu_macros::Model;
use kungfu_orm::{Db, DbConfig, Model as ModelTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

#[derive(Model, Serialize, Deserialize)]
#[table(name = "todos")]
struct Todo {
    #[field(primary, auto_increment)]
    id: i64,
    title: String,
    // SQLite stores booleans as 0/1 integers. We use i64 here for
    // compatibility — convert in the handler layer (0 = false, 1 = true).
    done: i64,
}

fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let rt = tokio::runtime::Builder::new_multi_thread().enable_all().build().unwrap();
    rt.block_on(async_main());
}

async fn async_main() {
    let db = Db::connect(&DbConfig {
        url: "sqlite::memory:".into(),
        max_connections: 5,
        min_connections: 1,
    }).await.expect("DB connect");

    for stmt in &kungfu_orm::generate_migration::<Todo>().up_sql {
        let _ = db.execute(stmt, &[]).await;
    }
    let _ = Todo { id: 0, title: "Learn Kungfu.js".into(), done: 0 }.insert(&db).await;
    let _ = Todo { id: 0, title: "Build something amazing".into(), done: 0 }.insert(&db).await;

    let db = Arc::new(db);
    let jwt = Arc::new(JwtService::new("todo-app-secret-key"));

    let css = kungfu_css::compile_classes("flex p-4 bg-blue-500 text-white rounded-lg text-xl font-bold w-full");
    let css_bytes = bytes::Bytes::from(css.into_bytes());

    Kungfu::new()
        .title("Kungfu Todo App")
        .handle_get("/kungfu.css", move |_req, _res| {
            kungfu_core::Response::new().header("content-type", "text/css").send_bytes(css_bytes.clone())
        })
        .handle_get("/", |_req, res| {
            res.html(r#"<!doctype html><html><head><meta charset="utf-8"><title>Kungfu Todo</title><link rel="stylesheet" href="/kungfu.css"></head><body style="margin:0;padding:2rem;font-family:sans-serif;background:#f3f4f6;"><div style="max-width:500px;margin:0 auto;background:white;padding:2rem;border-radius:8px;box-shadow:0 1px 3px rgba(0,0,0,0.1);"><h1>🥋 Kungfu Todo App</h1><p>Complete example: ORM + JWT + WebSocket + CSS</p><ul><li><a href="/docs">Swagger UI</a></li><li><code>POST /login</code> → get JWT</li><li><code>GET /todos</code> → list (needs JWT)</li><li><code>ws://localhost:3000/ws</code> → WebSocket echo</li></ul></div></body></html>"#)
        })
        .handle_post("/login", {
            let jwt = jwt.clone();
            move |req, res| {
                let body: serde_json::Value = req.json_value().unwrap_or(serde_json::json!({}));
                let username = body.get("username").and_then(|v| v.as_str()).unwrap_or("guest");
                let claims = serde_json::json!({"sub": username, "role": "user", "exp": 9999999999_i64});
                match jwt.sign(&claims) {
                    Ok(token) => res.json(&serde_json::json!({"token": token, "user": username})),
                    Err(e) => res.status(500).text(format!("JWT error: {e}")),
                }
            }
        })
        .use_middleware(auth_jwt(
            JwtConfig::new("todo-app-secret-key")
                .public_path("/login").public_path("/").public_path("/kungfu.css")
                .public_path("/docs").public_path("/openapi.json"),
        ))
        .handle_get_async("/todos", {
            let db = db.clone();
            move |_req, res| {
                let db = db.clone();
                async move {
                    match Todo::all(&db).await {
                        Ok(todos) => res.json(&todos),
                        Err(e) => res.status(500).text(format!("DB error: {e}")),
                    }
                }
            }
        })
        .handle_post_async("/todos", {
            let db = db.clone();
            move |req, res| {
                let db = db.clone();
                async move {
                    #[derive(Deserialize)]
                    struct CreateTodo { title: String }
                    let body: CreateTodo = match req.json() { Ok(b) => b, Err(e) => return res.error(e) };
                    let todo = Todo { id: 0, title: body.title, done: 0 };
                    match todo.insert(&db).await {
                        Ok(t) => res.status(201).json(&t),
                        Err(e) => res.status(500).text(format!("DB error: {e}")),
                    }
                }
            }
        })
        .handle_get_async("/todos/:id", {
            let db = db.clone();
            move |req, res| {
                let db = db.clone();
                async move {
                    let id: i64 = req.param("id").and_then(|s| s.parse().ok()).unwrap_or(0);
                    match Todo::find_by_pk(id, &db).await {
                        Ok(t) => res.json(&t),
                        Err(_) => res.status(404).text("Not found"),
                    }
                }
            }
        })
        .handle_put_async("/todos/:id", {
            let db = db.clone();
            move |req, res| {
                let db = db.clone();
                async move {
                    let id: i64 = req.param("id").and_then(|s| s.parse().ok()).unwrap_or(0);
                    #[derive(Deserialize)]
                    struct UpdateTodo { title: Option<String>, done: Option<i64> }
                    let body: UpdateTodo = match req.json() { Ok(b) => b, Err(e) => return res.error(e) };
                    let mut sets = Vec::new();
                    if let Some(t) = body.title { sets.push(("title", serde_json::json!(t))); }
                    if let Some(d) = body.done { sets.push(("done", serde_json::json!(d))); }
                    if sets.is_empty() { return res.status(400).text("No fields"); }
                    match Todo::update_by_pk(&db, id, sets).await {
                        Ok(n) => res.json(&serde_json::json!({"updated": n})),
                        Err(e) => res.status(500).text(format!("DB: {e}")),
                    }
                }
            }
        })
        .handle_delete_async("/todos/:id", {
            let db = db.clone();
            move |req, res| {
                let db = db.clone();
                async move {
                    let id: i64 = req.param("id").and_then(|s| s.parse().ok()).unwrap_or(0);
                    match Todo::delete_by_pk(id, &db).await {
                        Ok(n) => res.json(&serde_json::json!({"deleted": n})),
                        Err(e) => res.status(500).text(format!("DB: {e}")),
                    }
                }
            }
        })
        .handle_get_async("/todos/count", {
            let db = db.clone();
            move |_req, res| {
                let db = db.clone();
                async move {
                    match Todo::count(&db).await {
                        Ok(n) => res.json(&serde_json::json!({"count": n})),
                        Err(e) => res.status(500).text(format!("DB: {e}")),
                    }
                }
            }
        })
        .ws("/ws", |mut ws: WebSocket| async move {
            ws.send_text("🥋 Kungfu WebSocket connected!").await;
            while let Some(msg) = ws.recv().await {
                match msg {
                    WebSocketMessage::Text(t) => { ws.send_text(format!("echo: {t}")).await; }
                    WebSocketMessage::Close => break,
                    _ => {}
                }
            }
        })
        .run("0.0.0.0:3000").await.unwrap();
}
