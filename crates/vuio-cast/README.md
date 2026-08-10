# vuio-cast

Async Google Cast (Chromecast) client for Rust, built on [tokio](https://tokio.rs).
Discover, connect to, and control Cast devices, with TLS, heartbeats, reconnection
and request-response correlation handled for you.

Used by [VuIO](https://github.com/vuiodev/vuio) for Chromecast and Google TV playback.

## Credit

This is a fork of [oxicast](https://github.com/denniskribl/oxicast) by **Dennis Kribl**,
published under a distinct name so it cannot be confused with, or accidentally
substituted for, the original crate.

## Changes from upstream

- `tokio-rustls` uses `ring` only (`default-features = false`), so dependents are not
  forced to compile `aws-lc-sys` through Cargo feature unification.

## License

Dual-licensed under MIT / Apache-2.0, the same as upstream. See
[LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

Original project: https://github.com/denniskribl/oxicast
