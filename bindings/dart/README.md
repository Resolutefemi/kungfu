# kungfu (Dart)

> Dart binding for the Kungfu.js framework.

## Status: Scaffold

V1 ships a scaffold of the Dart binding using `dart:ffi` to call the C ABI.
The Dart FFI has a limitation that callbacks must be top-level static
functions (not closures), so per-route handler registration needs a
workaround planned for V1.1.

## Install

```yaml
# pubspec.yaml
dependencies:
  kungfu:
    path: /path/to/kungfu/bindings/dart
  ffi: ^2.0.0
```

## Quickstart (planned API)

```dart
import 'package:kungfu/kungfu.dart';

void main() {
  final app = Kungfu();
  app.get('/hello', (req, res) {
    res.status(200).json('{"message":"world"}');
  });
  app.listen(3000);
}
```

## License

MIT OR Apache-2.0.
