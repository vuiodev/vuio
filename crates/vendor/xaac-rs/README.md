# xaac-rs

[![Crates.io Version](https://img.shields.io/crates/v/xaac-rs)](https://crates.io/crates/xaac-rs)

High-level Rust bindings for AAC and xHE-AAC encoding and decoding built on top
of `libxaac-sys`.

An optional `python` feature also exposes the full high-level API as a PyO3
extension module importable as `xaac_rs`.

This crate wraps the low-level FFI API exposed by the `libxaac-sys`
crate and provides a safer, more idiomatic Rust interface for:

- AAC-LC encoding
- HE-AACv1 and HE-AACv2 encoding
- AAC-LD and AAC-ELD encoding
- USAC/xHE-AAC encoder configuration
- Streaming decoder initialization and frame decoding
- Decoder stream metadata and DRC status reporting
- Optional MPEG-D DRC sidecar handling for streams that expose DRC payloads

## Example

Basic AAC-LC ADTS encoding:

```rust
use xaac_rs::{Encoder, EncoderConfig, OutputFormat};

let mut encoder = Encoder::new(EncoderConfig {
    output_format: OutputFormat::Adts,
    ..EncoderConfig::default()
})?;

let pcm = vec![0i16; encoder.input_frame_bytes() / 2];
let packet = encoder.encode_i16_interleaved(&pcm)?;

assert!(!packet.data.is_empty());
# Ok::<(), xaac_rs::Error>(())
```

Chunked AAC decoding:

```rust
use xaac_rs::{DecodeStatus, Decoder, DecoderConfig};

let mut decoder = Decoder::new(DecoderConfig::default())?;
let data: &[u8] = &[];
match decoder.decode_stream_chunk(data)? {
    DecodeStatus::Frame(frame) => {
        assert!(!frame.pcm.is_empty());
    }
    DecodeStatus::NeedMoreInput(progress) => {
        assert!(!progress.initialized || progress.stream_info.is_some());
    }
    DecodeStatus::EndOfStream => {}
}
# Ok::<(), xaac_rs::Error>(())
```

## WAV Conversion Example

The crate includes a runnable example that converts a typical PCM WAV file to
AAC ADTS:

```bash
cargo run --example convert_wav_to_aac -- input.wav output.aac
cargo run --example convert_wav_to_aac -- input.wav output.aac 192000
```

The example:

- parses RIFF/WAVE input directly
- supports PCM WAV and `WAVE_FORMAT_EXTENSIBLE`
- supports 16-bit, 24-bit, and 32-bit PCM
- zero-pads the final partial frame before encoding

See [examples/convert_wav_to_aac.rs](examples/convert_wav_to_aac.rs).
The Python equivalent lives at [examples/python/convert_wav_to_aac.py](examples/python/convert_wav_to_aac.py).

## Decode Examples

Inspect stream metadata:

```bash
cargo run --example file_info -- input.aac
```

Decode a stream incrementally:

```bash
cargo run --example decode_stream -- input.aac
```

Python equivalents:

```bash
uv run python examples/python/file_info.py input.aac
uv run python examples/python/decode_stream.py input.aac
uv run python examples/python/convert_wav_to_aac.py input.wav output.aac
```

The decoder API now supports:

- `decode_stream_chunk` for incremental input
- `finish` to flush/end the stream explicitly
- `DecodeStatus` for `Frame`, `NeedMoreInput`, and `EndOfStream`
- richer `StreamInfo` reporting, including channel mode, DRC state, preroll, and gain payload metadata

Raw and `Mp4Raw` decoder modes remain explicit through `DecoderTransport` plus `RawStreamConfig`.

## Python Bindings

Build and install the Python module locally with maturin:

```bash
python3 -m venv .venv
.venv/bin/pip install maturin
VIRTUAL_ENV="$PWD/.venv" .venv/bin/maturin build --features python -i .venv/bin/python
.venv/bin/pip install target/wheels/xaac_rs-*.whl
```

Example:

```python
import xaac_rs

config = xaac_rs.EncoderConfig()
config.output_format = xaac_rs.OutputFormat.Adts
encoder = xaac_rs.Encoder(config)

pcm = [0] * (encoder.input_frame_bytes() // 2)
packet = encoder.encode_i16_interleaved(pcm)

decoder = xaac_rs.Decoder()
status = decoder.decode_stream_chunk(packet.data)
```

## Public API

Main exported types:

- `Encoder`
- `EncoderConfig`
- `Profile`
- `OutputFormat`
- `Decoder`
- `DecoderConfig`
- `DecoderDrcConfig`
- `DecodeStatus`
- `DecodeProgress`
- `DecodedFrame`
- `EncodedPacket`
- `EncodedFrame`
- `EncoderDrcConfig`
- `InverseQuantizationMode`
- `Error`

## Validation

Verified locally with:

```bash
cargo check
cargo test
cargo check --features python
cargo test --features python
cargo check --example convert_wav_to_aac
cargo check --example file_info
cargo check --example decode_stream

uv run python -m unittest discover -s python_tests -v
```
