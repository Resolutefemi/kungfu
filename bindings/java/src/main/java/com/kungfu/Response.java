package com.kungfu;

/**
 * HTTP response builder.
 *
 * <p>Usage:
 * <pre>{@code
 * res.status(200).header("x-foo", "bar").json("{\"ok\":true}");
 * }</pre>
 */
public class Response {
    private int status = 200;
    private final java.util.Map<String, String> headers = new java.util.HashMap<>();
    private String body = "";
    private String contentType = "text/plain; charset=utf-8";

    public Response status(int code) {
        this.status = code;
        return this;
    }

    public Response header(String key, String value) {
        this.headers.put(key, value);
        return this;
    }

    public Response text(String text) {
        this.body = text;
        this.contentType = "text/plain; charset=utf-8";
        return this;
    }

    public Response html(String html) {
        this.body = html;
        this.contentType = "text/html; charset=utf-8";
        return this;
    }

    public Response json(String json) {
        this.body = json;
        this.contentType = "application/json; charset=utf-8";
        return this;
    }

    // Getters used by the native side to construct the wire response.
    public int getStatus() { return status; }
    public String getBody() { return body; }
    public String getContentType() { return contentType; }
    public java.util.Map<String, String> getHeaders() { return headers; }
}
