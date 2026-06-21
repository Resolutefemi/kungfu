// Package kungfu provides an idiomatic Go binding for the Kungfu.js
// polyglot web framework. The Rust core is exposed via a C ABI (planned
// for V1.1); until the C ABI lands, this package wraps the HTTP server
// directly using Go's net/http as a fallback.
//
// Quickstart:
//
//	package main
//
//	import "github.com/kungfu-js/kungfu/bindings/go/kungfu"
//
//	func main() {
//	    app := kungfu.New()
//	    app.Get("/hello", func(w kungfu.ResponseWriter, r *kungfu.Request) {
//	        w.JSON(200, map[string]string{"message": "world"})
//	    })
//	    app.Run(":3000")
//	}
package kungfu

// Version of the Kungfu.js framework this binding targets.
const Version = "1.0.0"
