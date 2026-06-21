// Kungfu.java — Java binding entry point (JNI scaffold).
//
// In V1 this is a scaffold. The actual JNI native methods live in
// `KungfuNative.cpp` (to be implemented when the C ABI is stable).
// For now, Java users can call into the C ABI directly via JNA.

package com.kungfu;

import java.util.Map;
import java.util.HashMap;
import java.util.function.BiConsumer;

/**
 * Kungfu.js — Java binding.
 *
 * <p>Quickstart:
 * <pre>{@code
 * Kungfu app = new Kungfu();
 * app.get("/hello", (req, res) -> {
 *     res.status(200).json("{\"message\":\"world\"}");
 * });
 * app.listen(3000);
 * }</pre>
 */
public class Kungfu {
    static {
        // Load the native library. The library is `libkungfu_core.so` on Linux,
        // `libkungfu_core.dylib` on macOS, `kungfu_core.dll` on Windows.
        System.loadLibrary("kungfu_core");
    }

    private long routerPtr;  // opaque pointer to KungfuRouter

    public Kungfu() {
        this.routerPtr = nativeRouterNew();
    }

    /**
     * Register a GET route.
     */
    public void get(String path, BiConsumer<Request, Response> handler) {
        registerRoute(0, path, handler);
    }

    /**
     * Register a POST route.
     */
    public void post(String path, BiConsumer<Request, Response> handler) {
        registerRoute(1, path, handler);
    }

    /**
     * Register a PUT route.
     */
    public void put(String path, BiConsumer<Request, Response> handler) {
        registerRoute(2, path, handler);
    }

    /**
     * Register a DELETE route.
     */
    public void delete(String path, BiConsumer<Request, Response> handler) {
        registerRoute(3, path, handler);
    }

    /**
     * Start the server on the given port. Blocks the calling thread.
     */
    public void listen(int port) {
        nativeServerListen(routerPtr, port);
    }

    @Override
    protected void finalize() throws Throwable {
        if (routerPtr != 0) {
            nativeRouterFree(routerPtr);
            routerPtr = 0;
        }
        super.finalize();
    }

    private void registerRoute(int method, String path, BiConsumer<Request, Response> handler) {
        // V1: store handler in a static map keyed by path. The JNI callback
        // looks up the handler by path and invokes it.
        HandlerRegistry.register(path, handler);
        nativeRouterRegister(routerPtr, method, path);
    }

    // ─── Native methods (JNI) ─────────────────────────────────────────────────

    private static native long nativeRouterNew();
    private static native void nativeRouterFree(long routerPtr);
    private static native void nativeRouterRegister(long routerPtr, int method, String path);
    private static native void nativeServerListen(long routerPtr, int port);
}
