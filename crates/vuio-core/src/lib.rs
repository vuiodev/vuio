#![deny(unsafe_op_in_unsafe_fn)]
// Making the modules internal revealed ~110 items with no caller inside the
// crate: they existed only as public API. They are still reachable under
// `unstable-internals` (the tests use many of them), so the lint only fires in
// the default build. Silencing it here is deliberate and temporary — the real
// fix is to delete what is genuinely unused, which needs a per-item check on
// every target platform rather than a macOS-only build.
#![cfg_attr(not(feature = "unstable-internals"), allow(dead_code))]
#![deny(clippy::undocumented_unsafe_blocks)]

// Everything below is internal. `vuio-core` commits to the facade re-exported
// after this block and nothing else: a surface small enough to keep stable for
// the lifetime of a shipped device, which is the whole point of the crate.
// Hosts needing richer interaction drive the server over its HTTP and MCP APIs.
//
// The `unstable-internals` feature opens these modules so the integration tests
// (which exercise the DLNA, SSDP and database layers directly) can reach them.
// It carries no stability promise whatsoever and must never be enabled by a
// dependent crate.
macro_rules! internal_modules {
    ($($name:ident),* $(,)?) => {
        $(
            #[cfg(feature = "unstable-internals")]
            #[doc(hidden)]
            pub mod $name;
            #[cfg(not(feature = "unstable-internals"))]
            pub(crate) mod $name;
        )*
    };
}

internal_modules!(
    casting,
    config,
    database,
    error,
    lifecycle,
    logging,
    media,
    platform,
    runtime,
    runtime_state,
    state,
    ssdp,
    tv_control,
    watcher,
    web,
);

// ── The stable public API ──────────────────────────────────────────────────
pub use crate::error::{Error, ErrorKind, Result};
pub use crate::runtime::{Runtime, RuntimeHandle, RuntimeOptions, RuntimeStatus};

// The promise is not only which items exist but what they can do. Losing
// `Send`/`Sync` on the handle would stop hosts from holding it in shared state,
// and `Error` dropping `std::error::Error` would break every `?` in a caller —
// both are silent source changes that no signature diff would catch, so they
// are asserted at compile time here.
const _: () = {
    const fn shareable<T: Send + Sync + 'static>() {}
    const fn standard_error<T: std::error::Error + Send + Sync + 'static>() {}

    shareable::<Runtime>();
    shareable::<RuntimeHandle>();
    shareable::<RuntimeOptions>();
    shareable::<RuntimeStatus>();
    shareable::<Error>();
    shareable::<ErrorKind>();
    standard_error::<Error>();
};

#[cfg(feature = "unstable-internals")]
#[doc(hidden)]
pub type DefaultDatabase = crate::database::redb::RedbDatabase;
#[cfg(feature = "unstable-internals")]
#[doc(hidden)]
pub type DefaultAppState = crate::state::AppState<DefaultDatabase>;



/// Natural comparison for strings containing embedded numbers.
///
/// Numeric segments are compared by value so that, e.g., "s01e2" < "s01e10".
/// Non-numeric segments are compared case-insensitively.
pub(crate) fn natural_cmp(left: &str, right: &str) -> std::cmp::Ordering {
    use std::cmp::Ordering;

    let left = left.to_lowercase();
    let right = right.to_lowercase();
    let mut left_chars = left.chars().peekable();
    let mut right_chars = right.chars().peekable();

    loop {
        match (left_chars.peek(), right_chars.peek()) {
            (Some(a), Some(b)) if a.is_ascii_digit() && b.is_ascii_digit() => {
                let left_number: String = std::iter::from_fn(|| {
                    left_chars.next_if(|character| character.is_ascii_digit())
                })
                .collect();
                let right_number: String = std::iter::from_fn(|| {
                    right_chars.next_if(|character| character.is_ascii_digit())
                })
                .collect();
                let order = left_number
                    .trim_start_matches('0')
                    .len()
                    .cmp(&right_number.trim_start_matches('0').len())
                    .then_with(|| {
                        left_number
                            .trim_start_matches('0')
                            .cmp(right_number.trim_start_matches('0'))
                    })
                    .then_with(|| left_number.len().cmp(&right_number.len()));
                if order != Ordering::Equal {
                    return order;
                }
            }
            (Some(_), Some(_)) => {
                let order = left_chars.next().cmp(&right_chars.next());
                if order != Ordering::Equal {
                    return order;
                }
            }
            _ => return left_chars.next().cmp(&right_chars.next()),
        }
    }
}
