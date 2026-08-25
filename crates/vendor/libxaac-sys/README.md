# libxaac-sys

Rust FFI bindings for the vendored `libxaac` C library.

This crate exposes low-level bindings to the `libxaac` encoder and decoder APIs.
By default it builds the bundled upstream sources and does not require a system
installation of `libxaac`.

Upstream project: <https://github.com/ittiam-systems/libxaac>

## Features

- `bundled`:
  Build the vendored `libxaac` sources with CMake. Enabled by default.
- `static`:
  Prefer static linking. Enabled by default.
- `dynamic`:
  Prefer dynamic linking when using a system-provided `libxaac`.

`static` and `dynamic` are mutually exclusive.

## Linking Modes

Default:

```toml
[dependencies]
libxaac-sys = "0.1"
```

Bundled static build:

```toml
[dependencies]
libxaac-sys = { version = "0.1", features = ["bundled", "static"] }
```

System dynamic linking:

```toml
[dependencies]
libxaac-sys = { version = "0.1", default-features = false, features = ["dynamic"] }
```

System static linking:

```toml
[dependencies]
libxaac-sys = { version = "0.1", default-features = false, features = ["static"] }
```

## License

This crate is licensed under Apache-2.0. The vendored upstream `libxaac`
sources are included under their Apache-2.0 license in `libxaac/LICENSE`.
