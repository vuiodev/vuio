# vuio-codec-dts

Pure-Rust DTS audio decoder (Core profile) for VuIO.

Forked from [oxideav-dts](https://github.com/OxideAV/oxideav-dts) by Mark Karpeles, MIT licensed — see `LICENSE` and `NOTICE`.

## Overview

`vuio-codec-dts` provides a high-performance, 100% pure Rust DTS (DCA) audio stream decoder:
- Core subframe, subsubframe, and joint-subband decoding.
- 14-bit and 16-bit packed bitstream parsing.
- Integration with `vuio-codec-core` for unified sample, packet, and frame pipeline processing.

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](https://www.apache.org/licenses/LICENSE-2.0))
- MIT License ([LICENSE-MIT](https://opensource.org/licenses/MIT))

at your option.
