// Kungfu.swift — Swift binding for the Kungfu.js framework.
//
// Uses Swift's C interop to call the C ABI directly. The native library is
// `libkungfu_core.so` / `.dylib` / `.dll`, built with
// `cargo build -p kungfu-core --features ffi`.

import Foundation
#if canImport(Glibc)
import Glibc
#elseif canImport(Darwin)
import Darwin
#endif

// ─── C ABI imports ────────────────────────────────────────────────────────────

// Opaque pointer types matching `core/kungfu.h`.
public typealias KungfuRouterRef = OpaquePointer
public typealias KungfuServerRef = OpaquePointer
public typealias KungfuRequestRef = OpaquePointer
public typealias KungfuResponseRef = OpaquePointer

public typealias KungfuHandlerFn = @convention(c) (KungfuRequestRef?, KungfuResponseRef?) -> Void

@_silgen_name("kungfu_router_new")
public func kungfu_router_new() -> KungfuRouterRef?

@_silgen_name("kungfu_router_free")
public func kungfu_router_free(_ router: KungfuRouterRef?)

@_silgen_name("kungfu_router_get")
public func kungfu_router_get(_ router: KungfuRouterRef?, _ path: UnsafePointer<CChar>, _ handler: @convention(c) (KungfuRequestRef?, KungfuResponseRef?) -> Void)

@_silgen_name("kungfu_router_post")
public func kungfu_router_post(_ router: KungfuRouterRef?, _ path: UnsafePointer<CChar>, _ handler: @convention(c) (KungfuRequestRef?, KungfuResponseRef?) -> Void)

@_silgen_name("kungfu_server_new")
public func kungfu_server_new(_ router: KungfuRouterRef?) -> KungfuServerRef?

@_silgen_name("kungfu_server_listen")
public func kungfu_server_listen(_ server: KungfuServerRef?, _ port: Int32)

@_silgen_name("kungfu_request_param")
public func kungfu_request_param(_ req: KungfuRequestRef?, _ key: UnsafePointer<CChar>) -> UnsafePointer<CChar>?

@_silgen_name("kungfu_response_status")
public func kungfu_response_status(_ res: KungfuResponseRef?, _ code: Int32)

@_silgen_name("kungfu_response_json")
public func kungfu_response_json(_ res: KungfuResponseRef?, _ json: UnsafePointer<CChar>)

// ─── Swift API ────────────────────────────────────────────────────────────────

/// A Kungfu application.
public final class Kungfu {
    private var router: KungfuRouterRef?

    public init() {
        self.router = kungfu_router_new()
    }

    deinit {
        if let r = router { kungfu_router_free(r) }
    }

    public func get(_ path: String, handler: @escaping (Request, Response) -> Void) {
        path.withCString { cPath in
            // V1 scaffold: the C callback can't capture Swift closures
            // directly. We use a global handler table keyed by path.
            HandlerTable.shared.register(path, handler: handler)
            kungfu_router_get(router, cPath) { req, res in
                let request = Request(ptr: req!)
                let response = Response(ptr: res!)
                // Look up the handler by reading the request path.
                // For V1 simplicity, just call the first registered handler.
                if let h = HandlerTable.shared.handlers.first?.value {
                    h(request, response)
                }
            }
        }
    }

    public func listen(_ port: Int = 3000) {
        let server = kungfu_server_new(router)
        kungfu_server_listen(server, Int32(port))
    }
}

public struct Request {
    let ptr: KungfuRequestRef

    public func param(_ key: String) -> String {
        return key.withCString { cKey -> String in
            guard let cVal = kungfu_request_param(ptr, cKey) else { return "" }
            return String(cString: cVal)
        }
    }
}

public struct Response {
    let ptr: KungfuResponseRef

    public func status(_ code: Int) -> Response {
        kungfu_response_status(ptr, Int32(code))
        return self
    }

    public func json(_ json: String) {
        json.withCString { cJson in
            kungfu_response_json(ptr, cJson)
        }
    }
}

// V1: global handler table (workaround for C-callback-can't-capture-closure).
internal class HandlerTable {
    static let shared = HandlerTable()
    var handlers: [String: (Request, Response) -> Void] = [:]
    func register(_ path: String, handler: @escaping (Request, Response) -> Void) {
        handlers[path] = handler
    }
}
