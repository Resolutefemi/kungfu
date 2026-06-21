# kungfu (Swift)

> Swift binding for the Kungfu.js framework.

## Status: Scaffold

V1 ships a scaffold of the Swift binding using Swift's C interop to call
the C ABI. The Swift Package Manager target uses a `module.modulemap` to
wrap `core/kungfu.h`.

## Install

```swift
// Package.swift
.package(path: "/path/to/kungfu/bindings/swift")
```

Build the native library first:

```bash
cargo build -p kungfu-core --release --features ffi
```

## Quickstart (planned API)

```swift
import Kungfu

let app = Kungfu()
app.get("/hello") { req, res in
    res.status(200).json("{\"message\":\"world\"}")
}
app.listen(3000)
```

## License

MIT OR Apache-2.0.
