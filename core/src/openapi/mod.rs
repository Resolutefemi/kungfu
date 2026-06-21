//! Auto-OpenAPI generation.
//!
//! Walks the router's route table and produces an OpenAPI 3.1 document. The
//! spec is served at `/openapi.json`; a minimal Swagger UI is served at
//! `/docs`.

use std::sync::Arc;

use serde::Serialize;

use crate::error::Result;
use crate::request::{Method, Request};
use crate::response::Response;
use crate::router::{Handler, RouteMeta, Router};

#[derive(Debug, Serialize)]
pub struct OpenApiDoc {
    pub openapi: &'static str,
    pub info: OpenApiInfo,
    pub servers: Vec<OpenApiServer>,
    pub paths: std::collections::BTreeMap<String, OpenApiPathItem>,
}

#[derive(Debug, Serialize)]
pub struct OpenApiInfo {
    pub title: String,
    pub version: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
pub struct OpenApiServer {
    pub url: String,
    pub description: String,
}

#[derive(Debug, Serialize)]
#[allow(non_snake_case)]
pub struct OpenApiPathItem {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub get: Option<OpenApiOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub post: Option<OpenApiOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub put: Option<OpenApiOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub delete: Option<OpenApiOperation>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub patch: Option<OpenApiOperation>,
}

#[derive(Debug, Serialize)]
#[allow(non_snake_case)]
pub struct OpenApiOperation {
    pub summary: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tags: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requestBody: Option<OpenApiRequestBody>,
    pub responses: OpenApiResponses,
}

#[derive(Debug, Serialize)]
pub struct OpenApiRequestBody {
    pub required: bool,
    pub content: std::collections::BTreeMap<String, OpenApiMediaType>,
}

#[derive(Debug, Serialize)]
pub struct OpenApiMediaType {
    pub schema: serde_json::Value,
}

#[derive(Debug, Serialize)]
pub struct OpenApiResponses {
    #[serde(rename = "200")]
    pub ok: OpenApiResponse,
    #[serde(rename = "400")]
    pub bad_request: OpenApiResponse,
    #[serde(rename = "500")]
    pub internal: OpenApiResponse,
}

#[derive(Debug, Serialize)]
pub struct OpenApiResponse {
    pub description: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<std::collections::BTreeMap<String, OpenApiMediaType>>,
}

/// Convert a Kungfu path like `/users/:id` to an OpenAPI path like `/users/{id}`.
fn to_openapi_path(path: &str) -> String {
    let mut out = String::with_capacity(path.len() + 8);
    let mut iter = path.chars().peekable();
    while let Some(c) = iter.next() {
        if c == ':' {
            // read the param name
            let mut name = String::new();
            while let Some(&n) = iter.peek() {
                if n.is_alphanumeric() || n == '_' {
                    name.push(n);
                    iter.next();
                } else {
                    break;
                }
            }
            out.push('{');
            out.push_str(&name);
            out.push('}');
        } else if c == '*' {
            // wildcard — represent as {rest}
            let mut name = String::new();
            while let Some(&n) = iter.peek() {
                if n.is_alphanumeric() || n == '_' {
                    name.push(n);
                    iter.next();
                } else {
                    break;
                }
            }
            if name.is_empty() {
                name.push_str("rest");
            }
            out.push('{');
            out.push_str(&name);
            out.push('}');
        } else {
            out.push(c);
        }
    }
    out
}

fn build_operation(meta: &RouteMeta) -> OpenApiOperation {
    let request_body = meta.request_schema.as_ref().map(|schema| OpenApiRequestBody {
        required: true,
        content: {
            let mut map = std::collections::BTreeMap::new();
            map.insert(
                "application/json".to_string(),
                OpenApiMediaType {
                    schema: schema.clone(),
                },
            );
            map
        },
    });

    let mut ok_content: Option<std::collections::BTreeMap<String, OpenApiMediaType>> = None;
    if let Some(schema) = &meta.response_schema {
        let mut map = std::collections::BTreeMap::new();
        map.insert(
            "application/json".to_string(),
            OpenApiMediaType {
                schema: schema.clone(),
            },
        );
        ok_content = Some(map);
    }

    OpenApiOperation {
        summary: meta.summary.clone().unwrap_or_else(|| {
            format!("{} {}", meta.method.as_str(), meta.path)
        }),
        tags: meta.tags.clone(),
        requestBody: request_body,
        responses: OpenApiResponses {
            ok: OpenApiResponse {
                description: "Successful response".into(),
                content: ok_content,
            },
            bad_request: OpenApiResponse {
                description: "Validation error".into(),
                content: None,
            },
            internal: OpenApiResponse {
                description: "Internal server error".into(),
                content: None,
            },
        },
    }
}

/// Generate the OpenAPI document from a router's registered routes.
pub fn generate_spec(router: &Router, title: &str, version: &str) -> OpenApiDoc {
    let mut paths: std::collections::BTreeMap<String, OpenApiPathItem> = std::collections::BTreeMap::new();

    for meta in router.routes() {
        let oa_path = to_openapi_path(&meta.path);
        let entry = paths
            .entry(oa_path)
            .or_insert_with(|| OpenApiPathItem {
                get: None,
                post: None,
                put: None,
                delete: None,
                patch: None,
            });
        let op = build_operation(&meta);
        match meta.method {
            Method::Get => entry.get = Some(op),
            Method::Post => entry.post = Some(op),
            Method::Put => entry.put = Some(op),
            Method::Delete => entry.delete = Some(op),
            Method::Patch => entry.patch = Some(op),
            _ => {} // HEAD / OPTIONS not in OpenAPI path items by default
        }
    }

    OpenApiDoc {
        openapi: "3.1.0",
        info: OpenApiInfo {
            title: title.into(),
            version: version.into(),
            description: "Auto-generated by the Kungfu router.".into(),
        },
        servers: vec![OpenApiServer {
            url: "/".into(),
            description: "Local server".into(),
        }],
        paths,
    }
}

/// Register the OpenAPI + Swagger UI routes on a router.
/// Call this *after* registering all user routes so the spec captures them.
pub fn register_docs_routes(router: &mut Router, title: &str, version: &str) -> Result<()> {
    // Pre-compute the spec at registration time. (Hot-reload will invalidate this.)
    let spec = generate_spec(router, title, version);
    let spec_json = serde_json::to_string_pretty(&spec).unwrap();

    let spec_handler: Handler = {
        let spec_json = spec_json.clone();
        Arc::new(move |_req: Request| {
            let spec_json = spec_json.clone();
            Box::pin(async move {
                Response::new()
                    .header("content-type", "application/json; charset=utf-8")
                    .send(spec_json.into_bytes())
            })
        })
    };

    let swagger_handler: Handler = Arc::new(move |_req: Request| {
        Box::pin(async move {
            Response::new().html(SWAGGER_UI_HTML)
        })
    });

    router.add_with_meta(
        RouteMeta {
            path: "/openapi.json".into(),
            method: Method::Get,
            summary: Some("OpenAPI 3.1 specification".into()),
            tags: vec!["docs".into()],
            ..Default::default()
        },
        spec_handler,
    )?;
    router.add_with_meta(
        RouteMeta {
            path: "/docs".into(),
            method: Method::Get,
            summary: Some("Swagger UI".into()),
            tags: vec!["docs".into()],
            ..Default::default()
        },
        swagger_handler,
    )?;

    Ok(())
}

const SWAGGER_UI_HTML: &str = r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>Kungfu API Docs</title>
  <link rel="stylesheet" href="https://unpkg.com/swagger-ui-dist@5/swagger-ui.css">
  <style>body { margin: 0; }</style>
</head>
<body>
  <div id="swagger-ui"></div>
  <script src="https://unpkg.com/swagger-ui-dist@5/swagger-ui-bundle.js"></script>
  <script>
    window.addEventListener('DOMContentLoaded', () => {
      window.ui = SwaggerUIBundle({
        url: '/openapi.json',
        dom_id: '#swagger-ui',
        deepLinking: true,
      });
    });
  </script>
</body>
</html>"#;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::router::{Handler, Router};

    fn ok_handler() -> Handler {
        Arc::new(|_r| Box::pin(async { Response::new().text("ok") }))
    }

    #[test]
    fn generates_spec_for_registered_routes() {
        let mut r = Router::new();
        r.get("/users", ok_handler()).unwrap();
        r.get("/users/:id", ok_handler()).unwrap();
        r.post("/users", ok_handler()).unwrap();
        let spec = generate_spec(&r, "Test API", "1.0.0");
        assert_eq!(spec.info.title, "Test API");
        assert!(spec.paths.contains_key("/users"));
        assert!(spec.paths.contains_key("/users/{id}"));
        let users = spec.paths.get("/users").unwrap();
        assert!(users.get.is_some());
        assert!(users.post.is_some());
    }

    #[test]
    fn converts_wildcards_to_openapi_params() {
        assert_eq!(to_openapi_path("/assets/*path"), "/assets/{path}");
        assert_eq!(to_openapi_path("/users/:id/posts/:post_id"), "/users/{id}/posts/{post_id}");
    }
}
