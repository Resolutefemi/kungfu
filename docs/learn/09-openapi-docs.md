# OpenAPI & Auto Docs

> ⏱️ 4 minutes

Kungfu generates an OpenAPI 3.1 spec automatically from your route
definitions. No annotations needed — every route you register is
reflected in the spec.

## The Swagger UI

Start any Kungfu server and visit http://localhost:3000/docs. You'll see
Swagger UI with all your routes, ready to test interactively.

The raw spec is at http://localhost:3000/openapi.json.

## Adding metadata

For richer docs, register routes with `RouteMeta`:

```rust
use kungfu::{Kungfu, Method, RouteMeta, Handler};
use std::sync::Arc;

let handler: Handler = Arc::new(|_req| {
    Box::pin(async { kungfu::Response::new().text("hi") })
});

Kungfu::new()
    .add_with_meta(
        RouteMeta {
            path: "/hello".into(),
            method: Method::Get,
            summary: Some("Say hello".into()),
            tags: vec!["greeting".into()],
            ..Default::default()
        },
        handler,
    )
```

The summary and tags appear in the Swagger UI.

## Request/response schemas

You can attach JSON Schema to routes for richer OpenAPI documentation:

```rust
RouteMeta {
    path: "/users".into(),
    method: Method::Post,
    summary: Some("Create a user".into()),
    tags: vec!["users".into()],
    request_schema: Some(serde_json::json!({
        "type": "object",
        "properties": {
            "email": {"type": "string"},
            "password": {"type": "string", "minLength": 8}
        },
        "required": ["email", "password"]
    })),
    response_schema: Some(serde_json::json!({
        "type": "object",
        "properties": {
            "id": {"type": "integer"},
            "email": {"type": "string"}
        }
    })),
    ..Default::default()
}
```

When `request_schema` is set, the framework can validate the body
automatically using the `validate_json` middleware.

## Validating requests

```rust
use kungfu::middleware_builtin::validate_json;
use serde_json::json;

let user_schema = json!({
    "type": "object",
    "properties": {
        "email": {"type": "string"},
        "password": {"type": "string", "minLength": 8}
    },
    "required": ["email", "password"]
});

Kungfu::new()
    .use_middleware(validate_json("/users", Method::Post, user_schema))
    .handle_post("/users", |_req, res| res.text("created"))
```

If the body doesn't match the schema, the framework returns 422 with a
structured error message:

```json
{
  "error": {
    "code": 422,
    "message": "Validation failed",
    "detail": [
      "$.password: string too short (min 8 chars)"
    ],
    "suggestion": "Fix the listed fields and retry."
  }
}
```

## Disabling auto docs

If you don't want the `/openapi.json` and `/docs` endpoints:

```rust
Kungfu::new()
    .disable_auto_docs()
    .run("0.0.0.0:3000")
```

## Using the spec

The generated spec can be fed to other tools:

- **Client SDK generation**: `openapi-generator` produces TypeScript, Python,
  Go, etc. clients from the spec.
- **Postman import**: import the JSON into Postman for testing.
- **Documentation sites**: tools like Redoc render the spec as a static
  HTML site.

## Next steps

Continue to [Deployment](./10-deployment.md) to learn how to ship your
Kungfu app to production.
