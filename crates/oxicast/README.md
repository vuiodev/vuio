# oxicast (VuIO fork)

Modified fork of [oxicast](https://github.com/denniskribl/oxicast) by **Dennis Kribl**, used by VuIO for Chromecast / Google Cast control.

## Changes from upstream

- `tokio-rustls` uses `ring` only (`default-features = false`), so dependents are not forced to compile `aws-lc-sys` via Cargo feature unification.

## License

Dual-licensed under MIT / Apache-2.0 (same as upstream). See [LICENSE-MIT](LICENSE-MIT) and [LICENSE-APACHE](LICENSE-APACHE).

Original project: https://github.com/denniskribl/oxicast
