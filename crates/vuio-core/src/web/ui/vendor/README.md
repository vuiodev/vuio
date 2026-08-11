# Vendored dashboard assets

These files are third-party libraries checked in verbatim and embedded into the binary via
`include_str!` in `crates/vuio-core/src/web/ui.rs`. They are served from `/assets/{file}`.

They are vendored rather than loaded from a CDN because VuIO is expected to run on an
isolated LAN with no route to the internet. A CDN reference would leave the dashboard's
video player silently broken there.

| File | Upstream | Version | License |
|---|---|---|---|
| `plyr.min.js` | `plyr@3.8.4/dist/plyr.min.js` | 3.8.4 | MIT (`LICENSE-plyr`) |
| `plyr.css` | `plyr@3.8.4/dist/plyr.css` | 3.8.4 | MIT (`LICENSE-plyr`) |
| `plyr.svg` | `plyr@3.8.4/dist/plyr.svg` | 3.8.4 | MIT (`LICENSE-plyr`) |
| `hls.min.js` | `hls.js@1.6.17/dist/hls.min.js` | 1.6.17 | Apache-2.0 (`LICENSE-hls.js`) |
| `blank.mp4` | generated locally, see below | — | — |

Both licenses are compatible with this crate's `MIT OR Apache-2.0`.

## Local modifications

The trailing `//# sourceMappingURL=` comment was stripped from `plyr.min.js` and
`hls.min.js`. The `.map` files are not vendored, so the comment would only produce a 404 in
the browser's network log whenever DevTools is open.

Nothing else is modified. `plyr.css` contains no `url()` references at all.

## The two CDN defaults Plyr ships with

Both were confirmed by watching the network log, not by reading the docs.

`iconUrl` defaults to `https://cdn.plyr.io/3.8.4/plyr.svg`. Plyr only inlines the sprite over
AJAX when that URL is cross-origin; for a same-origin URL it leaves the `<use href>` for the
browser to resolve, which works. Left at the default, every control renders blank on an
offline LAN. `dashboard.html` passes `iconUrl: '/assets/plyr.svg'`.

`blankVideo` defaults to `https://cdn.plyr.io/static/blank.mp4`. Plyr's `destroy()` calls
`cancelRequests()`, which parks the media element on `blankVideo` to drop the open
connection — so *every closed player* fires a request at a third party. `dashboard.html`
passes `blankVideo: '/assets/blank.mp4'`.

`blank.mp4` is a 32x32 black frame, one 40 ms sample, no audio:

```sh
ffmpeg -f lavfi -i "color=c=black:s=32x32:d=0.04:r=25" \
       -c:v libx264 -pix_fmt yuv420p -movflags +faststart -an blank.mp4
```

## Refreshing

```sh
V=crates/vuio-core/src/web/ui/vendor
curl -fsSL https://cdn.jsdelivr.net/npm/plyr@3.8.4/dist/plyr.min.js  -o $V/plyr.min.js
curl -fsSL https://cdn.jsdelivr.net/npm/plyr@3.8.4/dist/plyr.css     -o $V/plyr.css
curl -fsSL https://cdn.jsdelivr.net/npm/plyr@3.8.4/dist/plyr.svg     -o $V/plyr.svg
curl -fsSL https://cdn.jsdelivr.net/npm/hls.js@1.6.17/dist/hls.min.js -o $V/hls.min.js
perl -0pi -e 's{\s*//# sourceMappingURL=\S+\s*$}{\n}' $V/plyr.min.js $V/hls.min.js
```

After refreshing, bump the `?v=` query strings in `dashboard.html` so caches are invalidated,
and re-check that `plyr.css` still has no `url()` references.
