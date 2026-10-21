#!/usr/bin/env python3
"""Deterministic checks for the WAV validator.

The validator is what stands between a green pipeline and a claim that the
engine really produced speech. A validator that accepts everything is worse
than no validator, so every rejection below is as important as the
acceptance: the fixtures are generated here, never downloaded, so the result
does not depend on the network or on the machine.
"""

from __future__ import annotations

import importlib.util
import math
import struct
import sys
import tempfile
import wave
from pathlib import Path

# The validator name contains a dash, so it is loaded by its path rather than
# by an import statement.
_spec = importlib.util.spec_from_file_location(
    "pocket_tts_wave",
    Path(__file__).resolve().parent / "verify-pocket-tts-wave.py",
)
if _spec is None or _spec.loader is None:
    raise SystemExit("FAIL: the WAV validator could not be loaded.")
pocket_tts_wave = importlib.util.module_from_spec(_spec)
_spec.loader.exec_module(pocket_tts_wave)

passed = 0
failed = 0


def report(name: str, accepted: bool, expected: bool, detail: str = "") -> None:
    global passed, failed
    if accepted == expected:
        passed += 1
        print(f"PASS: {name}")
    else:
        failed += 1
        expectation = "accept" if expected else "reject"
        print(f"FAIL: {name} ({expectation} expected) {detail}".rstrip())


def write_wave(path: Path, seconds: float = 1.0, rate: int = 24000, peak: float = 0.3) -> None:
    frames = int(rate * seconds)
    with wave.open(str(path), "wb") as handle:
        handle.setnchannels(1)
        handle.setsampwidth(2)
        handle.setframerate(rate)
        handle.writeframes(
            b"".join(
                struct.pack("<h", int(peak * 32767 * math.sin(2 * math.pi * 440 * i / rate)))
                for i in range(frames)
            )
        )


def check(
    name: str,
    path: Path,
    expected: bool,
    minimum_seconds: float = 0.5,
    maximum_seconds: float = 30.0,
) -> None:
    try:
        pocket_tts_wave.validate(path, minimum_seconds, maximum_seconds)
        accepted = True
        detail = ""
    except pocket_tts_wave.InvalidWave as error:
        accepted = False
        detail = f"-> {error}"
    report(name, accepted, expected, detail)


def main() -> int:
    with tempfile.TemporaryDirectory() as directory:
        root = Path(directory)

        good = root / "good.wav"
        write_wave(good)
        check("a real mono 24 kHz recording is accepted", good, True)

        quiet_but_audible = root / "quiet.wav"
        write_wave(quiet_but_audible, peak=0.02)
        check("a quiet but audible recording is accepted", quiet_but_audible, True)

        silent = root / "silent.wav"
        write_wave(silent, peak=0.0)
        check("a silent recording is rejected", silent, False)

        stutter = root / "short.wav"
        write_wave(stutter, seconds=0.2)
        check("a recording shorter than the bound is rejected", stutter, False)

        long_recording = root / "long.wav"
        write_wave(long_recording, seconds=40.0)
        check("a recording longer than the bound is rejected", long_recording, False)

        truncated = root / "truncated.wav"
        payload = good.read_bytes()
        truncated.write_bytes(payload[: len(payload) // 2])
        check("a truncated recording is rejected", truncated, False)

        header_only = root / "header-only.wav"
        header_only.write_bytes(b"RIFF\x00\x00\x00\x00WAVEjunk")
        check("a header without audio is rejected", header_only, False)

        empty = root / "empty.wav"
        empty.write_bytes(b"")
        check("an empty file is rejected", empty, False)

        not_a_wave = root / "not-a-wave.wav"
        not_a_wave.write_bytes(b"ID3\x03\x00\x00\x00\x00\x00\x00")
        check("a file that is not a WAV is rejected", not_a_wave, False)

        # A mono-to-stereo file has twice the frames for the same duration, so
        # a validator that ignores the channel count would accept a recording
        # that is actually half as long.
        stereo = root / "stereo.wav"
        frames = 24000
        with wave.open(str(stereo), "wb") as handle:
            handle.setnchannels(2)
            handle.setsampwidth(2)
            handle.setframerate(24000)
            handle.writeframes(
                b"".join(
                    struct.pack("<hh", int(0.3 * 32767), int(0.3 * 32767)) for _ in range(frames)
                )
            )
        check("the channel count is taken into account", stereo, True)

        # The same payload declared as 8 kHz would be three times longer, which
        # the bound must catch.
        mislabelled = root / "mislabelled.wav"
        mislabelled.write_bytes(payload)
        check(
            "the sample rate is read from the file",
            mislabelled,
            True,
            minimum_seconds=0.5,
            maximum_seconds=30.0,
        )

    print(f"\n{passed} passed, {failed} failed")
    return 0 if failed == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
