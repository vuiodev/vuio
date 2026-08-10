# vuio-core

The embeddable VuIO media server runtime: configuration, media database and
indexing, SSDP discovery, UPnP ContentDirectory, HTTP range streaming, casting,
and the management API — startable from inside a host application with no
process, no command line, and no signal handling of its own.

```rust,no_run
use vuio_core::{Runtime, RuntimeOptions};

#[tokio::main]
async fn main() -> vuio_core::Result<()> {
    let vuio = Runtime::start(
        RuntimeOptions::new()
            .media_dir("/srv/media")
            .port(8080)
            .server_name("Lounge"),
    );

    vuio.wait().await
}
```

The binary `vuio` command-line server is a separate package, `vuio-cli`, built on
this crate. Embedding does not require it.

## Stability

`vuio-core` is built for devices — NAS boxes, routers, SBCs, vehicle and vessel
installations — whose firmware is updated rarely, and sometimes never. A
breaking change reaches those installations slowly or not at all, so this crate
promises a surface small enough to keep for the life of the hardware.

### What is covered

Everything reachable from the crate root with default features:

| Item | |
| --- | --- |
| `Runtime` | starts a server |
| `RuntimeHandle` | status, shutdown, wait |
| `RuntimeOptions` | how to start it |
| `RuntimeStatus` | what it is doing |
| `Error`, `ErrorKind`, `Result` | how it fails |

Within a major release these keep their names, their signatures, and their
behaviour, and they stay `Send + Sync + 'static`. Nothing here names a type from
another crate, so a dependency's major release cannot break your build: an
error's cause is a boxed `std::error::Error`, and shutdown is driven through
`RuntimeHandle` rather than a foreign cancellation type.

Richer interaction is deliberately not offered as Rust types. Hosts that need to
browse, search, or control the server drive its HTTP and MCP APIs, which is what
the VuIO dashboard and `vuio-tower` already do, and which works from any
language.

### What is not covered

- **`unstable-internals`.** This feature opens the crate's internal modules so
  the integration tests can drive the DLNA, SSDP and database layers directly.
  It is `#[doc(hidden)]`, it carries no stability promise of any kind, and a
  dependent crate must never enable it. Anything it exposes can change or vanish
  in a patch release.
- **On-the-wire and on-disk formats** are versioned by their own protocols
  (SSDP, UPnP, the redb schema), not by this crate's version.
- **Log output** — message text and structure are for operators, not parsers.

### Rust version

`rust-version` in `Cargo.toml` is the supported minimum, tested in CI. Raising
it is a **minor** release, never a patch, so a pinned `=x.y.z` never moves under
a fixed toolchain.

### Features

Features are additive: enabling one never removes an item or changes behaviour
for code that did not ask for it. Everything ships **on by default** — the full
server is the product — and the flags exist so a constrained deployment can opt
out of what it will never use, and so a vendor auditing what ships in their
firmware has less to read.

| Feature | Gives up when off | Crates |
| --- | --- | --- |
| `casting` | Chromecast, AirPlay and DLNA renderer control | 63 |
| `metadata` | tags and embedded cover art (files keep filename titles) | 13 |
| `diagnostics` | system and disk metrics on the status endpoints | 1 |
| `dashboard` | the built-in web UI | 0 |
| `mcp` | the Model Context Protocol server | 0 |

Counts are crates removed from the dependency graph for
`aarch64-unknown-linux-musl`, measured with `cargo tree`. The default build
resolves **217** crates and `--no-default-features` resolves **141**. The last
two shed compiled code rather than dependencies.

What is never gated, because it is what a media server *is*: SSDP discovery,
UPnP ContentDirectory, HTTP range streaming, media scanning and indexing, the
database, and configuration.

### Versioning

Semantic versioning, with the covered surface above as the subject. Additions
are minor; anything that could stop a compiling caller from compiling is major.
Two gates in CI enforce this rather than trusting review: a committed snapshot
of the public API (`api-surface.txt`) that must be updated deliberately, and
`cargo semver-checks` against the previous release.

## License

MIT OR Apache-2.0
