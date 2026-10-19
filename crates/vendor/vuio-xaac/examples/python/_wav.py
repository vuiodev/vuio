from __future__ import annotations

from dataclasses import dataclass


PCM_SUBFORMAT_GUID = bytes(
    [
        0x01,
        0x00,
        0x00,
        0x00,
        0x00,
        0x00,
        0x10,
        0x00,
        0x80,
        0x00,
        0x00,
        0xAA,
        0x00,
        0x38,
        0x9B,
        0x71,
    ]
)


@dataclass(frozen=True)
class WavFile:
    sample_rate: int
    channels: int
    bits_per_sample: int
    channel_mask: int | None
    pcm_data: bytes

    @classmethod
    def parse(cls, data: bytes) -> "WavFile":
        if len(data) < 12 or data[0:4] != b"RIFF" or data[8:12] != b"WAVE":
            raise ValueError("input is not a RIFF/WAVE file")

        offset = 12
        channels = None
        sample_rate = None
        bits_per_sample = None
        channel_mask = None
        pcm_data = None

        while offset + 8 <= len(data):
            chunk_id = data[offset : offset + 4]
            chunk_size = int.from_bytes(data[offset + 4 : offset + 8], "little")
            offset += 8

            if offset + chunk_size > len(data):
                raise ValueError("WAV chunk extends past end of file")

            chunk = data[offset : offset + chunk_size]
            if chunk_id == b"fmt ":
                if len(chunk) < 16:
                    raise ValueError("WAV fmt chunk is too short")

                audio_format = int.from_bytes(chunk[0:2], "little")
                channels = int.from_bytes(chunk[2:4], "little")
                sample_rate = int.from_bytes(chunk[4:8], "little")
                bits_per_sample = int.from_bytes(chunk[14:16], "little")

                if audio_format == 1:
                    pass
                elif audio_format == 0xFFFE:
                    if len(chunk) < 40:
                        raise ValueError("WAV extensible fmt chunk is too short")
                    channel_mask = int.from_bytes(chunk[20:24], "little")
                    valid_bits = int.from_bytes(chunk[18:20], "little")
                    if valid_bits != 0:
                        bits_per_sample = valid_bits
                    if chunk[24:40] != PCM_SUBFORMAT_GUID:
                        raise ValueError("only PCM WAV extensible files are supported")
                else:
                    raise ValueError("only uncompressed PCM WAV files are supported")
            elif chunk_id == b"data":
                pcm_data = chunk

            offset += chunk_size
            if chunk_size % 2 == 1:
                offset += 1

        if channels is None:
            raise ValueError("WAV fmt chunk missing channel count")
        if sample_rate is None:
            raise ValueError("WAV fmt chunk missing sample rate")
        if bits_per_sample is None:
            raise ValueError("WAV fmt chunk missing bits per sample")
        if pcm_data is None:
            raise ValueError("WAV data chunk not found")
        if bits_per_sample not in (16, 24, 32):
            raise ValueError(
                "only 16-bit, 24-bit, and 32-bit PCM WAV files are supported"
            )

        bytes_per_sample = bits_per_sample // 8
        frame_size = channels * bytes_per_sample
        if frame_size == 0 or len(pcm_data) % frame_size != 0:
            raise ValueError("WAV data size is not aligned to full PCM frames")

        return cls(
            sample_rate=sample_rate,
            channels=channels,
            bits_per_sample=bits_per_sample,
            channel_mask=channel_mask,
            pcm_data=pcm_data,
        )
