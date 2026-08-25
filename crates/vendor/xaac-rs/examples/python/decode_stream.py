#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

import xaac_rs


def usage() -> str:
    return "usage: python examples/python/decode_stream.py <input.aac>"


def main(argv: list[str]) -> int:
    if len(argv) != 2:
        raise ValueError(usage())

    data = Path(argv[1]).read_bytes()
    decoder = xaac_rs.Decoder()
    frame_count = 0
    pcm_bytes = 0
    offset = 0

    while offset < len(data):
        end = min(offset + 256, len(data))
        status = decoder.decode_stream_chunk(data[offset:end])
        if status.is_frame:
            frame_count += 1
            pcm_bytes += len(status.frame.pcm)
            print(
                f"frame {frame_count}: {len(status.frame.pcm)} pcm bytes, "
                f"{status.frame.stream_info.sample_rate} Hz, "
                f"{status.frame.stream_info.channels} ch"
            )
        elif status.is_need_more_input:
            progress = status.progress
            if progress.stream_info is not None:
                print(
                    "need more input: "
                    f"initialized={progress.initialized}, "
                    f"{progress.stream_info.sample_rate} Hz, "
                    f"{progress.stream_info.channels} ch"
                )
            else:
                print(f"need more input: initialized={progress.initialized}")
        else:
            break
        offset = end

    while True:
        status = decoder.finish()
        if status.is_frame:
            frame_count += 1
            pcm_bytes += len(status.frame.pcm)
            print(
                f"flush frame {frame_count}: {len(status.frame.pcm)} pcm bytes, "
                f"{status.frame.stream_info.sample_rate} Hz, "
                f"{status.frame.stream_info.channels} ch"
            )
            continue
        if status.is_need_more_input:
            continue
        break

    print(f"decoded frames: {frame_count}")
    print(f"decoded pcm bytes: {pcm_bytes}")
    return 0


if __name__ == "__main__":
    try:
        raise SystemExit(main(sys.argv))
    except Exception as exc:
        print(exc, file=sys.stderr)
        raise SystemExit(1)
