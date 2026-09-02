# vuio-cli

Command-line application and daemon for the [VuIO](https://github.com/vuiodev/vuio) media server.

## Installation

```bash
cargo install vuio-cli
```

This installs the `vuio` binary into your Cargo binary directory.

## Usage

### Start Server
```bash
vuio start
```

### Scan Media Libraries
```bash
vuio scan
```

### Device and Streaming Control
```bash
vuio devices
vuio play <device-id> <media-path>
```

## License

Licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](https://www.apache.org/licenses/LICENSE-2.0))
- MIT License ([LICENSE-MIT](https://opensource.org/licenses/MIT))

at your option.
