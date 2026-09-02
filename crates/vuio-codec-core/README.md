# vuio-codec-core

Core types and registries for VuIO audio codecs — timestamps, packets, audio frames, and codec registries.

Forked from [oxideav-core](https://github.com/OxideAV/oxideav-core) by Mark Karpeles, MIT licensed — see `LICENSE` and `NOTICE`.

## Overview

`vuio-codec-core` defines the foundational interfaces used across VuIO's audio processing and transcoding components:

- **Timestamps and Timebases**: Nanosecond-accurate time conversion and PTS/DTS manipulation.
- **Buffers and Slices**: Zero-allocation audio buffer management and arenas.
- **Audio Primitives**: Sample formats, channel layouts, audio parameters, and packet wrappers.
- **Registry System**: Dynamic registration and lookup for decoders, demuxers, and codec factories.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](https://www.apache.org/licenses/LICENSE-2.0))
- MIT License ([LICENSE-MIT](https://opensource.org/licenses/MIT))

at your option.
