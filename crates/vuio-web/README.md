# vuio-web

The VuIO browser interface, compiled into the server binary.

This crate is only the Rust half, and it is deliberately thin. The application
itself is a Svelte 5 / SvelteKit single-page app developed in its own
repository, [vuiodev/vuio-web](https://github.com/vuiodev/vuio-web) — working on
the interface needs no Rust, no `cargo` and no checkout of this one.

What lives here is the built bundle, `dist/`, committed. `build.rs` walks it and
generates an `include_bytes!` table, so building VuIO never requires Node; the
Docker builder stage has none.

`BUILD_INFO.toml` records the vuio-web commit `dist/` was built from. To refresh
it, clone vuio-web beside this repository and run `scripts/build-web.sh` from
the repository root.

## What it serves

`routes()` returns an `axum::Router` generic over the state it is merged into:

- `/` — the HTML shell.
- every file under `dist/`, at its own path. Hashed bundles under
  `/_app/immutable/` are served `immutable` for a year; everything else
  revalidates against an ETag over the compiled-in bytes.
- a fallback that answers an unknown path with the shell, which is what makes
  client-side routing work — except under a server prefix (`/api/`, `/media/`,
  …), where it answers 404 rather than handing back HTML that some `res.json()`
  will choke on later.

It is a fallback rather than a `/{*path}` route so that merging this into a
server cannot shadow one of that server's endpoints.

`vuio-core` merges it behind the same management auth as the dashboard and
serves it on the secondary listener, port 8090 by default.

## License

Dual-licensed under either of:

- [Apache License, Version 2.0](../../LICENSE-APACHE)
- [MIT License](../../LICENSE-MIT)

at your option.
