package com.kungfu;

import java.util.Map;
import java.util.concurrent.ConcurrentHashMap;
import java.util.function.BiConsumer;

/**
 * Static registry that maps route paths to their Java handlers.
 * The JNI native code looks up handlers here when a request arrives.
 *
 * <p>(V1 limitation: routes must be uniquely keyed by path. V1.1 will
 * support multiple methods per path.)
 */
public class HandlerRegistry {
    private static final Map<String, BiConsumer<Request, Response>> handlers = new ConcurrentHashMap<>();

    public static void register(String path, BiConsumer<Request, Response> handler) {
        handlers.put(path, handler);
    }

    public static BiConsumer<Request, Response> get(String path) {
        return handlers.get(path);
    }

    /**
     * Called from JNI when a request arrives. Builds the Request, invokes
     * the handler, and returns the Response.
     */
    public static Response dispatch(String path, String method, String body,
                                    Map<String, String> params,
                                    Map<String, String> headers,
                                    Map<String, String> query) {
        Request req = new Request(method, path, body, params, headers, query);
        Response res = new Response();
        BiConsumer<Request, Response> handler = handlers.get(path);
        if (handler != null) {
            handler.accept(req, res);
        } else {
            res.status(404).text("Not Found");
        }
        return res;
    }
}
