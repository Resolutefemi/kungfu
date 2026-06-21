// Hello-world example using the Kungfu C++ binding.
//
// Build:
//   clang++ -std=c++17 -I../../core -L../../target/release -lkungfu_core \
//     -o hello examples/hello.cpp
// (Make sure `kungfu_core` is built as a cdylib with `--features ffi`.)

#include "kungfu.hpp"
#include <iostream>

int main() {
    kungfu::KungfuRouter router;

    router.get("/hello", [](kungfu::Request& req, kungfu::Response& res) {
        res.status(200).json(R"({"message":"world","lang":"c++"})");
    });

    router.get("/", [](kungfu::Request& req, kungfu::Response& res) {
        res.status(200).html(
            "<h1>Hello from Kungfu (C++)!</h1>"
            "<p>Try <a href=\"/hello\">/hello</a></p>"
        );
    });

    std::cout << "🥋 Kungfu (C++) listening on http://localhost:3000" << std::endl;
    kungfu::KungfuServer server(std::move(router));
    server.listen(3000);
    return 0;
}
