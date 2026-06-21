// kungfu.dart — Dart binding for the Kungfu.js framework.
//
// Uses dart:ffi to call the C ABI. The native library is `libkungfu_core.so`
// (Linux), `libkungfu_core.dylib` (macOS), or `kungfu_core.dll` (Windows),
// built with `cargo build -p kungfu-core --features ffi`.

import 'dart:ffi';
import 'dart:io';
import 'package:ffi/ffi.dart';

// ─── Native bindings ──────────────────────────────────────────────────────────

typedef _NativeRouterNew = Pointer<Void> Function();
typedef _DartRouterNew = Pointer<Void> Function();

typedef _NativeRouterFree = Void Function(Pointer<Void>);
typedef _DartRouterFree = void Function(Pointer<Void>);

typedef _NativeRouterGet = Void Function(Pointer<Void>, Pointer<Utf8>, Pointer<NativeFunction<_NativeHandler>>);
typedef _DartRouterGet = void Function(Pointer<Void>, Pointer<Utf8>, Pointer<NativeFunction<_NativeHandler>>);

typedef _NativeHandler = Void Function(Pointer<NativeRequest>, Pointer<NativeResponse>);
typedef _DartHandler = void Function(Pointer<NativeRequest>, Pointer<NativeResponse>);

typedef _NativeServerListen = Void Function(Pointer<Void>, Int32);
typedef _DartServerListen = void Function(Pointer<Void>, int);

typedef _NativeRequestParam = Pointer<Utf8> Function(Pointer<NativeRequest>, Pointer<Utf8>);
typedef _DartRequestParam = Pointer<Utf8> Function(Pointer<NativeRequest>, Pointer<Utf8>);

typedef _NativeResponseStatus = Void Function(Pointer<NativeResponse>, Int32);
typedef _DartResponseStatus = void Function(Pointer<NativeResponse>, int);

typedef _NativeResponseJson = Void Function(Pointer<NativeResponse>, Pointer<Utf8>);
typedef _DartResponseJson = void Function(Pointer<NativeResponse>, Pointer<Utf8>);

// Opaque structs — we only need the pointer types.
class NativeRequest extends Opaque {}
class NativeResponse extends Opaque {}

/// Loads the native Kungfu library.
final DynamicLibrary _lib = _loadLib();

DynamicLibrary _loadLib() {
  if (Platform.isLinux) return DynamicLibrary.open('libkungfu_core.so');
  if (Platform.isMacOS) return DynamicLibrary.open('libkungfu_core.dylib');
  if (Platform.isWindows) return DynamicLibrary.open('kungfu_core.dll');
  throw UnsupportedError('Unsupported platform');
}

// ─── Dart API ─────────────────────────────────────────────────────────────────

class Kungfu {
  late final Pointer<Void> _router;

  Kungfu() {
    final newFn = _lib.lookupFunction<_NativeRouterNew, _DartRouterNew>('kungfu_router_new');
    _router = newFn();
  }

  void get(String path, void Function(Request, Response) handler) =>
      _register('kungfu_router_get', path, handler);

  void post(String path, void Function(Request, Response) handler) =>
      _register('kungfu_router_post', path, handler);

  void put(String path, void Function(Request, Response) handler) =>
      _register('kungfu_router_put', path, handler);

  void delete(String path, void Function(Request, Response) handler) =>
      _register('kungfu_router_delete', path, handler);

  void listen(int port) {
    final listenFn = _lib.lookupFunction<_NativeServerListen, _DartServerListen>('kungfu_server_listen');
    listenFn(_router, port);
  }

  void _register(String symbol, String path, void Function(Request, Response) handler) {
    final fn = _lib.lookupFunction<_NativeRouterGet, _DartRouterGet>(symbol);
    final pathPtr = path.toNativeUtf8();
    // V1 limitation: we don't yet wire per-route handlers through dart:ffi.
    // For now, this is a scaffold. The lookup of native callbacks requires
    // `Pointer.fromFunction`, which needs a top-level function — not a
    // closure. This is a known Dart FFI limitation we'll work around in V1.1.
    // fn(_router, pathPtr, ...);
    calloc.free(pathPtr);
    // Stash the handler so it doesn't get GC'd.
    _handlers[path] = handler;
  }

  // Stash handlers (V1 scaffold).
  static final Map<String, void Function(Request, Response)> _handlers = {};
}

class Request {
  final Pointer<NativeRequest> _ptr;
  Request(this._ptr);

  String param(String key) {
    final fn = _lib.lookupFunction<_NativeRequestParam, _DartRequestParam>('kungfu_request_param');
    final keyPtr = key.toNativeUtf8();
    final resultPtr = fn(_ptr, keyPtr);
    calloc.free(keyPtr);
    final value = resultPtr.address == 0 ? '' : resultPtr.toDartString();
    return value;
  }
}

class Response {
  final Pointer<NativeResponse> _ptr;
  Response(this._ptr);

  Response status(int code) {
    final fn = _lib.lookupFunction<_NativeResponseStatus, _DartResponseStatus>('kungfu_response_status');
    fn(_ptr, code);
    return this;
  }

  void json(String jsonStr) {
    final fn = _lib.lookupFunction<_NativeResponseJson, _DartResponseJson>('kungfu_response_json');
    final ptr = jsonStr.toNativeUtf8();
    fn(_ptr, ptr);
    calloc.free(ptr);
  }
}
