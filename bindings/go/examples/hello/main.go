// Hello-world example using the Kungfu Go binding.
//
// Run with:
//   cd bindings/go
//   go run ./examples/hello
package main

import (
	"encoding/json"

	"github.com/kungfu-js/kungfu/bindings/go/kungfu"
)

func main() {
	app := kungfu.New()

	app.Get("/hello", func(w kungfu.ResponseWriter, r *kungfu.Request) {
		w.JSON(200, map[string]interface{}{
			"message":   "world",
			"framework": "kungfu",
			"lang":      "go",
		})
	})

	app.Post("/echo/:name", func(w kungfu.ResponseWriter, r *kungfu.Request) {
		var body interface{}
		_ = json.Unmarshal(r.Body, &body)
		w.JSON(200, map[string]interface{}{
			"hello":    r.Params["name"],
			"you_sent": body,
		})
	})

	app.Get("/", func(w kungfu.ResponseWriter, r *kungfu.Request) {
		w.HTML(200, "<h1>Hello from Kungfu (Go)!</h1><p>Try /hello or POST /echo/yourname</p>")
	})

	println("🥋 Kungfu (Go) listening on http://localhost:3000")
	if err := app.Run(":3000"); err != nil {
		panic(err)
	}
}
