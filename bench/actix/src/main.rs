//! Actix-web hello-world server, for direct comparison with kungfu's bench.
//!
//! Run with:
//!   cargo run -p kungfu-bench-actix --release
//! Then drive it with `wrk` or `oha`:
//!   oha -z 5s -c 64 http://localhost:3001/hello

use actix_web::{web, App, HttpServer, Responder};
use serde_json::json;

async fn hello() -> impl Responder {
    web::Json(json!({"message":"world"}))
}

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    println!("actix-web bench listening on http://127.0.0.1:3001");
    HttpServer::new(|| App::new().route("/hello", web::get().to(hello)))
        .bind("127.0.0.1:3001")?
        .run()
        .await
}
