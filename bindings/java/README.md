# kungfu (Java)

> One API surface, infinite languages. The Java binding for the Kungfu.js framework.

## Status: Scaffold

V1 ships a scaffold of the Java binding. The actual JNI implementation
requires the C ABI (`kungfu.h`) plus a C++ JNI bridge — currently a TODO.
For now, Java users can call into the C ABI directly via [JNA](https://github.com/java-native-access/jna).

## Install

```bash
# Build the native library with the ffi feature:
cargo build -p kungfu-core --release --features ffi

# The Java side is plain Maven/Gradle. Copy the .java files into your
# project under src/main/java/com/kungfu/, and ensure the native library
# is on java.library.path.
```

## Quickstart

```java
import com.kungfu.Kungfu;
import com.kungfu.Request;
import com.kungfu.Response;

public class Main {
    public main() {
        Kungfu app = new Kungfu();
        app.get("/hello", (req, res) -> {
            res.status(200).json("{\"message\":\"world\"}");
        });
        app.listen(3000);
    }
}
```

## License

MIT OR Apache-2.0.
