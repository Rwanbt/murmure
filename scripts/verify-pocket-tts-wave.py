#!/usr/bin/env python3
"""Structural validator for the WAV files that the speech engine produces.

A file that is merely named `.wav`, or that carries the right magic bytes and
nothing else, is not evidence that the engine synthesized speech. This
validator walks the RIFF chunks by their declared sizes, checks that the
format is the one the engine promises, and inspects the decoded samples so an
empty or silent file cannot pass.

It is the POSIX counterpart of `verify-pocket-tts-wave.ps1`: the same
contract, so a file that is accepted on Windows is accepted here and a file
that is rejected there is rejected here too.
"""

from __future__ import annotations

import argparse
import struct
import sys
from pathlib import Path


class InvalidWave(Exception):
    """The file is not a WAV the engine is allowed to produce."""


def _chunks(data: bytes):
    """Yields `(identifier, payload)` for every chunk, honouring the padding."""
    if len(data) < 12:
        raise InvalidWave("the file is shorter than a RIFF header")
    if data[0:4] != b"RIFF":
        raise InvalidWave("the file does not start with the RIFF signature")
    if data[8:12] != b"WAVE":
        raise InvalidWave("the RIFF form type is not WAVE")

    declared = struct.unpack_from("<I", data, 4)[0]
    # The declared size excludes the eight bytes of 'RIFF' and the size field.
    if declared + 8 > len(data):
        raise InvalidWave(
            f"the RIFF header declares {declared + 8} bytes but the file holds {len(data)}"
        )

    offset = 12
    while offset + 8 <= len(data):
        identifier = data[offset : offset + 4]
        size = struct.unpack_from("<I", data, offset + 4)[0]
        start = offset + 8
        end = start + size
        if end > len(data):
            raise InvalidWave(
                f"the chunk {identifier!r} declares {size} bytes but only {len(data) - start} remain"
            )
        yield identifier, data[start:end]
        # A chunk is padded to an even length; the padding is not part of it.
        offset = end + (size % 2)


def validate(path: Path, minimum_seconds: float, maximum_seconds: float) -> str:
    data = path.read_bytes()
    if not data:
        raise InvalidWave("the file is empty")

    fmt = None
    samples = None
    for identifier, payload in _chunks(data):
        if identifier == b"fmt ":
            if len(payload) < 16:
                raise InvalidWave("the fmt chunk is shorter than the format it declares")
            audio_format, channels, rate, _, block_align, bits = struct.unpack_from(
                "<HHIIHH", payload, 0
            )
            fmt = {
                "format": audio_format,
                "channels": channels,
                "rate": rate,
                "block_align": block_align,
                "bits": bits,
            }
        elif identifier == b"data":
            samples = payload

    if fmt is None:
        raise InvalidWave("the file carries no fmt chunk")
    if samples is None:
        raise InvalidWave("the file carries no data chunk")
    # 1 is linear PCM and 3 is IEEE float; anything else is not a format the
    # engine is allowed to emit.
    if fmt["format"] not in (1, 3):
        raise InvalidWave(f"the audio format {fmt['format']} is not PCM or float")
    if fmt["channels"] < 1:
        raise InvalidWave("the file declares no audio channel")
    if fmt["rate"] < 8000:
        raise InvalidWave(f"the sample rate {fmt['rate']} Hz is too low for speech")
    if not samples:
        raise InvalidWave("the data chunk holds no sample")

    if fmt["bits"] == 16 and fmt["format"] == 1:
        count = len(samples) // 2
        if count == 0:
            raise InvalidWave("the data chunk holds no complete sample")
        values = struct.unpack_from(f"<{count}h", samples, 0)
        peak = max(abs(value) for value in values) / 32768.0
        frames = count
    elif fmt["bits"] == 32 and fmt["format"] == 3:
        count = len(samples) // 4
        if count == 0:
            raise InvalidWave("the data chunk holds no complete sample")
        values = struct.unpack_from(f"<{count}f", samples, 0)
        peak = max(abs(value) for value in values)
        frames = count
    else:
        raise InvalidWave(
            f"the sample format {fmt['bits']} bits / format {fmt['format']} is not supported"
        )

    seconds = frames / (fmt["rate"] * fmt["channels"])
    if seconds < minimum_seconds:
        raise InvalidWave(f"the recording lasts {seconds:.2f} s, less than {minimum_seconds} s")
    if seconds > maximum_seconds:
        raise InvalidWave(f"the recording lasts {seconds:.2f} s, more than {maximum_seconds} s")
    # A silent file has the right structure and no speech in it.
    if peak < 0.01:
        raise InvalidWave(f"the signal peaks at {peak:.4f}, which is silence")

    return (
        f"{fmt['rate']} Hz, {fmt['channels']} channel(s), {fmt['bits']} bits, "
        f"{seconds:.2f} s, peak {peak:.3f}"
    )


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--path", required=True, type=Path)
    parser.add_argument("--minimum-seconds", type=float, default=0.5)
    parser.add_argument("--maximum-seconds", type=float, default=30.0)
    arguments = parser.parse_args()

    try:
        report = validate(arguments.path, arguments.minimum_seconds, arguments.maximum_seconds)
    except (InvalidWave, OSError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1

    print(f"PASS: {report}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
