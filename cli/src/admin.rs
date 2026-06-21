//! `kungfu generate admin` — generate a static admin dashboard.
//!
//! Produces a self-contained HTML file at `public/admin/index.html` that
//! lists all routes from the OpenAPI spec and provides simple CRUD forms
//! for each. The dashboard reads `/openapi.json` at runtime.

pub fn generate_admin_html(api_title: &str) -> String {
    format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <meta name="viewport" content="width=device-width, initial-scale=1">
  <title>{title} — Admin Dashboard</title>
  <style>
    * {{ box-sizing: border-box; }}
    body {{ margin: 0; font-family: -apple-system, sans-serif; background: #f3f4f6; }}
    .header {{ background: #1f2937; color: white; padding: 1rem 2rem; }}
    .header h1 {{ margin: 0; font-size: 1.25rem; }}
    .container {{ display: grid; grid-template-columns: 240px 1fr; min-height: calc(100vh - 60px); }}
    .sidebar {{ background: white; padding: 1rem; border-right: 1px solid #e5e7eb; }}
    .sidebar h2 {{ font-size: 0.75rem; text-transform: uppercase; color: #6b7280; margin: 1rem 0 0.5rem; }}
    .sidebar a {{ display: block; padding: 0.5rem; color: #374151; text-decoration: none; border-radius: 4px; font-size: 0.875rem; }}
    .sidebar a:hover {{ background: #f3f4f6; }}
    .main {{ padding: 2rem; overflow: auto; }}
    .card {{ background: white; border-radius: 8px; padding: 1.5rem; margin-bottom: 1rem; box-shadow: 0 1px 3px rgba(0,0,0,0.1); }}
    .card h3 {{ margin: 0 0 1rem; font-size: 1rem; }}
    .method {{ display: inline-block; padding: 2px 8px; border-radius: 4px; font-size: 0.75rem; font-weight: 600; margin-right: 0.5rem; }}
    .method.get {{ background: #dbeafe; color: #1e40af; }}
    .method.post {{ background: #d1fae5; color: #065f46; }}
    .method.put {{ background: #fef3c7; color: #92400e; }}
    .method.delete {{ background: #fee2e2; color: #991b1b; }}
    .method.patch {{ background: #f3e8ff; color: #6b21a8; }}
    .empty {{ text-align: center; color: #6b7280; padding: 3rem; }}
    input, textarea, select {{ width: 100%; padding: 0.5rem; border: 1px solid #d1d5db; border-radius: 4px; font: inherit; }}
    button {{ padding: 0.5rem 1rem; background: #3b82f6; color: white; border: none; border-radius: 4px; cursor: pointer; font: inherit; }}
    button:hover {{ background: #2563eb; }}
    pre {{ background: #f3f4f6; padding: 1rem; border-radius: 4px; overflow: auto; font-size: 0.75rem; }}
  </style>
</head>
<body>
  <div class="header">
    <h1>🥋 {title} — Admin</h1>
  </div>
  <div class="container">
    <div class="sidebar">
      <h2>Routes</h2>
      <div id="routes-list">Loading...</div>
    </div>
    <div class="main">
      <div class="card">
        <h3>Welcome</h3>
        <p>This admin dashboard reads the OpenAPI spec at <code>/openapi.json</code> and lists all routes.</p>
        <p>Click a route on the left to see its details and try it interactively.</p>
      </div>
      <div id="route-detail"></div>
    </div>
  </div>
  <script>
    async function loadSpec() {{
      const resp = await fetch('/openapi.json');
      const spec = await resp.json();
      const list = document.getElementById('routes-list');
      list.innerHTML = '';
      Object.entries(spec.paths || {{}}).forEach(([path, methods]) => {{
        Object.entries(methods).forEach(([method, op]) => {{
          const a = document.createElement('a');
          a.href = '#';
          a.innerHTML = '<span class="method ' + method + '">' + method.toUpperCase() + '</span> ' + path;
          a.onclick = (e) => {{
            e.preventDefault();
            showRoute(path, method, op);
          }};
          list.appendChild(a);
        }});
      }});
      if (list.children.length === 0) {{
        list.innerHTML = '<div class="empty">No routes</div>';
      }}
    }}
    function showRoute(path, method, op) {{
      const detail = document.getElementById('route-detail');
      detail.innerHTML = `
        <div class="card">
          <h3><span class="method ${{method}}">${{method.toUpperCase()}}</span> ${{path}}</h3>
          <p>${{op.summary || ''}}</p>
          ${{op.tags && op.tags.length ? '<p>Tags: ' + op.tags.join(', ') + '</p>' : ''}}
        </div>
        <div class="card">
          <h3>Try it</h3>
          <form id="try-form">
            <p><label>Path parameters:</label><div id="path-params"></div></p>
            <p><label>Request body (JSON):</label><textarea id="req-body" rows="6" placeholder='${{op.requestBody ? '{{}}' : '(no body expected)'}}'></textarea></p>
            <p><button type="submit">Send</button></p>
          </form>
          <div id="resp" style="margin-top: 1rem;"></div>
        </div>
      `;
      // Extract path params.
      const params = path.match(/:[^/]+|\\{{[^}}]+\\}}/g) || [];
      const ppDiv = document.getElementById('path-params');
      params.forEach(p => {{
        const name = p.replace(/^[:{{]|}}$/g, '');
        ppDiv.innerHTML += `<input placeholder="${{name}}" id="param-${{name}}" style="margin-bottom: 0.5rem;">`;
      }});
      // Wire up form.
      document.getElementById('try-form').onsubmit = async (e) => {{
        e.preventDefault();
        let url = path;
        params.forEach(p => {{
          const name = p.replace(/^[:{{]|}}$/g, '');
          const v = document.getElementById('param-' + name).value;
          url = url.replace(p, v);
        }});
        const body = document.getElementById('req-body').value;
        const resp = await fetch(url, {{
          method: method.toUpperCase(),
          body: body || undefined,
          headers: body ? {{'Content-Type': 'application/json'}} : undefined,
        }});
        const text = await resp.text();
        document.getElementById('resp').innerHTML = `<p>Status: ${{resp.status}}</p><pre>${{text}}</pre>`;
      }};
    }}
    loadSpec();
  </script>
</body>
</html>"#,
        title = api_title
    )
}
