#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

import xaac_rs

from _wav import WavFile


def usage() -> str:
    return (
        "usage: python examples/python/convert_wav_to_aac.py "
        "<input.wav> <output.aac> [bitrate]"
    )


def main(argv: list[str]) -> int:
    if len(argv) not in (3, 4):
        raise ValueError(usage())

    input_path = Path(argv[1])
    output_path = Path(argv[2])
    bitrate = int(argv[3]) if len(argv) == 4 else 128_000

    wav = WavFile.parse(input_path.read_bytes())

    config = xaac_rs.EncoderConfig()
    config.profile = xaac_rs.Profile.AacLc
    config.sample_rate = wav.sample_rate
    config.native_sample_rate = wav.sample_rate
    config.channels = wav.channels
    config.channel_mask = wav.channel_mask or 0
    config.bitrate = bitrate
    config.pcm_word_size = wav.bits_per_sample
    config.output_format = xaac_rs.OutputFormat.Adts

    encoder = xaac_rs.Encoder(config)
    frame_bytes = encoder.input_frame_bytes()

    with output_path.open("wb") as output_file:
        offset = 0
        while offset + frame_bytes <= len(wav.pcm_data):
            packet = encoder.encode_pcm_bytes(
                wav.pcm_data[offset : offset + frame_bytes]
            )
            output_file.write(packet.data)
            offset += frame_bytes

        if offset < len(wav.pcm_data):
            frame = encoder.encode_pcm_bytes_with_padding(wav.pcm_data[offset:])
            output_file.write(frame.packet.data)

    print(
        f"encoded {wav.sample_rate} Hz, {wav.channels} channels, "
        f"{wav.bits_per_sample}-bit PCM to ADTS AAC at {bitrate} bps",
        file=sys.stderr,
    )
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv))
    except Exception as exc:
        print(exc, file=sys.stderr)
        raise SystemExit(1)
