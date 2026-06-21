package com.kungfu;

import java.util.Map;
import java.util.HashMap;

/**
 * HTTP request received by a Kungfu handler.
 */
public class Request {
    private final Map<String, String> params = new HashMap<>();
    private final Map<String, String> headers = new HashMap<>();
    private final Map<String, String> query = new HashMap<>();
    private final String body;
    private final String method;
    private final String path;

    public Request(String method, String path, String body,
                   Map<String, String> params, Map<String, String> headers,
                   Map<String, String> query) {
        this.method = method;
        this.path = path;
        this.body = body;
        this.params.putAll(params);
        this.headers.putAll(headers);
        this.query.putAll(query);
    }

    public String method() { return method; }
    public String path() { return path; }
    public String body() { return body; }
    public String param(String key) { return params.get(key); }
    public String header(String key) { return headers.get(key.toLowerCase()); }
    public String query(String key) { return query.get(key); }
}
