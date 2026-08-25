#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

import xaac_rs


def usage() -> str:
    return "usage: python examples/python/file_info.py <input.aac>"


def optional_value(value: object | None) -> str:
    return "unavailable" if value is None else str(value)


def estimate_adts_bitrate(
    data: bytes, sample_rate: int, sbr_mode: xaac_rs.SbrMode
) -> int | None:
    if sample_rate == 0:
        return None

    offset = 0
    frames = 0
    payload_bytes = 0

    while offset + 7 <= len(data):
        if data[offset] != 0xFF or (data[offset + 1] & 0xF0) != 0xF0:
            return None

        frame_length = (
            ((data[offset + 3] & 0x03) << 11)
            | (data[offset + 4] << 3)
            | ((data[offset + 5] & 0xE0) >> 5)
        )
        if frame_length < 7 or offset + frame_length > len(data):
            return None

        frames += 1
        payload_bytes += frame_length
        offset += frame_length

    if frames == 0 or offset != len(data):
        return None

    if sbr_mode == xaac_rs.SbrMode.Enabled:
        samples_per_frame = 2048
    elif sbr_mode == xaac_rs.SbrMode.Esbr:
        samples_per_frame = 4096
    else:
        samples_per_frame = 1024

    total_samples = frames * samples_per_frame
    return (payload_bytes * 8 * sample_rate) // total_samples


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        raise ValueError(usage())

    input_path = Path(argv[1])
    data = input_path.read_bytes()

    decoder = xaac_rs.Decoder()
    version = decoder.version()

    print(f"file: {input_path}")
    print(f"size: {len(data)} bytes")
    print(f"decoder: {version.name} {version.version}")
    print(f"decoder input capacity: {decoder.input_capacity()} bytes")

    if not data:
        print("stream info: file is empty")
        return 0

    probe_len = len(data)
    try:
        info = decoder.probe_stream_info(data)
    except Exception as exc:
        print("stream info: unavailable")
        print(f"decoder error: {exc}")
        print(f"probe bytes: {probe_len}")
        return 0

    print(f"probe bytes: {probe_len}")
    print(f"sample rate: {info.sample_rate} Hz")
    print(f"channels: {info.channels}")
    print(f"channel mask: 0x{info.channel_mask:x}")
    print(f"channel mode: {info.channel_mode}")
    print(f"pcm word size: {info.pcm_word_size} bits")
    print(f"audio object type: {info.audio_object_type}")
    print(f"sbr mode: {info.sbr_mode}")
    print(f"drc active: {info.drc_active}")
    print(f"drc target loudness: {optional_value(info.drc_target_loudness)}")
    print(f"drc loudness norm: {optional_value(info.drc_loudness_norm)}")
    print(f"loudness leveling: {optional_value(info.loudness_leveling)}")
    print(f"preroll frames: {optional_value(info.preroll_frames)}")
    print(f"gain payload bytes: {optional_value(info.gain_payload_len)}")

    bitrate = estimate_adts_bitrate(data, info.sample_rate, info.sbr_mode)
    if bitrate is None:
        print("bit rate: unavailable")
    else:
        print(f"bit rate: {bitrate} bps")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv))
    except Exception as exc:
        print(exc, file=sys.stderr)
        raise SystemExit(1)
